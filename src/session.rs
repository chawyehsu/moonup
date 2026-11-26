//! The per-invocation session.
//!
//! A session is created once per CLI invocation in `cli::start()` and passed
//! as `&Session` to every subcommand. It resolves moonup's locations once at
//! creation, owns the invocation's distribution-server HTTP client (so
//! downloads share pooled connections instead of paying a fresh handshake per
//! file), and caches parsed index metadata so repeated reads within one
//! invocation hit memory.
//!
//! The cache carries no invalidation machinery: every value fetched or
//! re-read during the invocation is stored back, making the cache the source
//! of truth for the session's lifetime — the same write-through shape the
//! index layer applies to disk. Index files that the release-ordering module
//! reads directly from disk (best-effort, offline) are never stored, so a
//! stale disk copy cannot pin the session's view or suppress a freshness
//! re-fetch.
//!
//! Sessions never observe each other's state: each invocation, including
//! each shim run, starts from disk state. The shim binary itself links none
//! of this module — it consumes only the std-only
//! [`paths`](crate::paths) layer.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use reqwest_middleware::ClientWithMiddleware;

use crate::dist_server::schema::{ChannelIndex, ChannelName, ComponentIndex, Index, Release};
use crate::layout::Layout;
use crate::toolchain::index;

/// The per-invocation context resolving locations once and serving index
/// metadata from memory.
pub struct Session {
    layout: Layout,
    http_client: OnceLock<ClientWithMiddleware>,
    indexes: Mutex<IndexCache>,
}

/// The in-memory index cache, keyed by index kind.
#[derive(Default)]
struct IndexCache {
    main: Option<Arc<Index>>,
    channels: HashMap<String, Arc<ChannelIndex>>,
    components: HashMap<(String, String), Arc<ComponentIndex>>,
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Session {
    /// Create a session, resolving the layout once.
    pub fn new() -> Self {
        Self {
            layout: Layout::from_env(),
            http_client: OnceLock::new(),
            indexes: Mutex::new(IndexCache::default()),
        }
    }

    /// The locations this session resolved at creation.
    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    /// The invocation's distribution-server HTTP client, built lazily and
    /// reused across every request the invocation makes.
    pub fn http_client(&self) -> ClientWithMiddleware {
        self.http_client
            .get_or_init(crate::utils::build_http_client_with_retry)
            .clone()
    }

    /// Read the main index, from memory when this session already loaded it.
    pub async fn read_index(&self) -> miette::Result<Arc<Index>> {
        if let Some(cached) = self.cached_main_index() {
            return Ok(cached);
        }

        let index = Arc::new(index::load_index(self).await?);
        if let Ok(mut cache) = self.indexes.lock() {
            cache.main = Some(Arc::clone(&index));
        }
        Ok(index)
    }

    /// Read a channel index, from memory when this session already loaded it.
    pub async fn read_channel_index(&self, channel: &str) -> miette::Result<Arc<ChannelIndex>> {
        if let Some(cached) = self.cached_channel_index(channel) {
            return Ok(cached);
        }

        let index = Arc::new(index::load_channel_index(self, channel).await?);
        if let Ok(mut cache) = self.indexes.lock() {
            cache
                .channels
                .insert(channel.to_owned(), Arc::clone(&index));
        }
        Ok(index)
    }

    /// Read a component index, from memory when this session already loaded
    /// it.
    pub async fn read_component_index(
        &self,
        channel: &str,
        release: &Release,
    ) -> miette::Result<Arc<ComponentIndex>> {
        let key = (channel.to_owned(), release.key().to_owned());
        if let Some(cached) = self.cached_component_index(&key) {
            return Ok(cached);
        }

        let index = Arc::new(index::load_component_index(self, channel, release).await?);
        if let Ok(mut cache) = self.indexes.lock() {
            cache.components.insert(key, Arc::clone(&index));
        }
        Ok(index)
    }

    /// The channels advertised by a readable main-index catalog.
    ///
    /// `None` means no usable channel catalog was available. `Some(vec![])`
    /// means a catalog was read but contained no channels.
    pub async fn available_channels(&self) -> miette::Result<Vec<ChannelName>> {
        Ok(self
            .read_index()
            .await?
            .channels()
            .iter()
            .map(|channel| channel.name.clone())
            .collect())
    }

    /// The cached main index, if this session already loaded it.
    ///
    /// Synchronous view for the release-ordering module, which is offline
    /// and best-effort.
    pub(crate) fn cached_main_index(&self) -> Option<Arc<Index>> {
        let cache = self.indexes.lock().ok()?;
        cache.main.clone()
    }

    /// The cached channel index for `channel`, if this session already
    /// loaded it.
    pub(crate) fn cached_channel_index(&self, channel: &str) -> Option<Arc<ChannelIndex>> {
        let cache = self.indexes.lock().ok()?;
        cache.channels.get(channel).cloned()
    }

    /// The cached component index for `(channel, release)`, if this session
    /// already loaded it.
    fn cached_component_index(&self, key: &(String, String)) -> Option<Arc<ComponentIndex>> {
        let cache = self.indexes.lock().ok()?;
        cache.components.get(key).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_resolves_paths_from_env_at_creation() {
        let tempdir = assert_fs::TempDir::new().expect("should create tempdir");
        temp_env::with_var(
            crate::constant::ENVNAME_MOONUP_HOME,
            Some(tempdir.path().as_os_str()),
            || {
                let session = Session::new();
                assert_eq!(session.layout().moonup_home(), tempdir.path());

                // a session created under a different env override keeps the
                // paths it resolved at creation
                let other = assert_fs::TempDir::new().expect("should create tempdir");
                temp_env::with_var(
                    crate::constant::ENVNAME_MOONUP_HOME,
                    Some(other.path().as_os_str()),
                    || {
                        let session2 = Session::new();
                        assert_eq!(session2.layout().moonup_home(), other.path());
                        assert_eq!(session.layout().moonup_home(), tempdir.path());
                    },
                );
            },
        );
    }
}

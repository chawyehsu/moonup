//! The per-invocation session.
//!
//! A session is created once per CLI invocation in `cli::start()` and passed
//! as `&Session` to every subcommand. It resolves moonup's locations once,
//! owns the invocation's distribution-server HTTP client (so downloads share
//! pooled connections instead of paying a fresh handshake per file), and
//! caches parsed index metadata so repeated reads within one invocation hit
//! memory.
//!
//! The cache carries no invalidation machinery: each entry is bound to the
//! on-disk `.lock` timestamp of the file it came from — the same oracle the
//! disk cache uses. Fresh reads ([`Session::read_index`]) serve a cached
//! entry only while that timestamp still matches disk and passes the
//! freshness window, so entries written by offline reads can be reused
//! without ever serving stale data as fresh. Offline reads
//! ([`Session::read_index_offline`], used by release ordering) serve memory
//! or disk and never perform a network request.
//!
//! Sessions never observe each other's state: each invocation, including
//! each shim run, starts from disk state. The shim binary itself links none
//! of this module — it consumes only the std-only
//! [`layout`](crate::layout) layer.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use jiff::Timestamp;
use reqwest_middleware::ClientWithMiddleware;

use crate::dist_server::schema::{
    ChannelIndex, ChannelName, ComponentIndex, Index, Release, Target,
};
use crate::layout::Layout;
use crate::toolchain::index;

/// The per-invocation context resolving locations once and serving index
/// metadata from memory.
pub struct Session {
    layout: OnceLock<Layout>,
    http_client: OnceLock<ClientWithMiddleware>,
    indexes: Mutex<IndexCache>,
}

/// One cached index value, bound to the on-disk lock timestamp of the file
/// it was read from (or that was just written for it). `None` means the file
/// carried no readable lock timestamp at read time.
struct Entry<T> {
    value: Arc<T>,
    lock: Option<Timestamp>,
}

impl<T> Clone for Entry<T> {
    fn clone(&self) -> Self {
        Entry {
            value: Arc::clone(&self.value),
            lock: self.lock,
        }
    }
}

/// The in-memory index cache, keyed by index kind.
#[derive(Default)]
struct IndexCache {
    main: Option<Entry<Index>>,
    channels: HashMap<String, Entry<ChannelIndex>>,
    components: HashMap<(String, String), Entry<ComponentIndex>>,
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Session {
    /// Create a session
    pub fn new() -> Self {
        Self {
            layout: OnceLock::new(),
            http_client: OnceLock::new(),
            indexes: Mutex::new(IndexCache::default()),
        }
    }

    /// The layout resolved for this session.
    pub fn layout(&self) -> &Layout {
        self.layout.get_or_init(Layout::from_env)
    }

    /// The invocation's distribution-server HTTP client, built lazily and
    /// reused across every request the invocation makes.
    pub fn http_client(&self) -> ClientWithMiddleware {
        self.http_client
            .get_or_init(crate::utils::build_http_client_with_retry)
            .clone()
    }

    /// Read the main index, enforcing the freshness rule: serve from memory
    /// or disk while the on-disk lock timestamp is fresh, fetch (and write
    /// back to disk and memory) when it is stale.
    pub async fn read_index(&self) -> miette::Result<Arc<Index>> {
        if let Some(cached) = self.indexes.lock().ok().and_then(|c| c.main.clone()) {
            let disk = index::read_lock_timestamp(&self.layout().index_path());
            if entry_is_fresh(&cached, disk) {
                return Ok(cached.value);
            }
        }

        let (index, lock) = index::load_index(self).await?;
        let value = Arc::new(index);
        if let Ok(mut cache) = self.indexes.lock() {
            cache.main = Some(Entry {
                value: Arc::clone(&value),
                lock: Some(lock),
            });
        }
        Ok(value)
    }

    /// Read the main index without any network access: serve memory or disk,
    /// freshness ignored. Best-effort — `None` when neither is readable.
    pub async fn read_index_offline(&self) -> Option<Arc<Index>> {
        if let Some(cached) = self.indexes.lock().ok().and_then(|c| c.main.clone()) {
            return Some(cached.value);
        }

        let (index, lock) = index::load_index_offline(self.layout()).await.ok()?;
        let value = Arc::new(index);
        if let Ok(mut cache) = self.indexes.lock() {
            cache.main = Some(Entry {
                value: Arc::clone(&value),
                lock,
            });
        }
        Some(value)
    }

    /// Read a channel index, enforcing the freshness rule (see
    /// [`Session::read_index`]).
    pub async fn read_channel_index(&self, channel: &str) -> miette::Result<Arc<ChannelIndex>> {
        if let Some(cached) = self
            .indexes
            .lock()
            .ok()
            .and_then(|c| c.channels.get(channel).cloned())
        {
            let disk = index::read_lock_timestamp(&self.layout().channel_index_path(channel));
            if entry_is_fresh(&cached, disk) {
                return Ok(cached.value);
            }
        }

        let (index, lock) = index::load_channel_index(self, channel).await?;
        let value = Arc::new(index);
        if let Ok(mut cache) = self.indexes.lock() {
            cache.channels.insert(
                channel.to_owned(),
                Entry {
                    value: Arc::clone(&value),
                    lock: Some(lock),
                },
            );
        }
        Ok(value)
    }

    /// Read a channel index without any network access (see
    /// [`Session::read_index_offline`]).
    pub async fn read_channel_index_offline(&self, channel: &str) -> Option<Arc<ChannelIndex>> {
        if let Some(cached) = self
            .indexes
            .lock()
            .ok()
            .and_then(|c| c.channels.get(channel).cloned())
        {
            return Some(cached.value);
        }

        let (index, lock) = index::load_channel_index_offline(self.layout(), channel)
            .await
            .ok()?;
        let value = Arc::new(index);
        if let Ok(mut cache) = self.indexes.lock() {
            cache.channels.insert(
                channel.to_owned(),
                Entry {
                    value: Arc::clone(&value),
                    lock,
                },
            );
        }
        Some(value)
    }

    /// Read a component index, enforcing the freshness rule (see
    /// [`Session::read_index`]).
    pub async fn read_component_index(
        &self,
        channel: &str,
        release: &Release,
    ) -> miette::Result<Arc<ComponentIndex>> {
        let key = (channel.to_owned(), release.key().to_owned());
        if let Some(cached) = self
            .indexes
            .lock()
            .ok()
            .and_then(|c| c.components.get(&key).cloned())
        {
            let target = Target::from_host()?.to_string();
            let disk = index::read_lock_timestamp(
                &self.layout().component_index_path(&key.0, &key.1, &target),
            );
            if entry_is_fresh(&cached, disk) {
                return Ok(cached.value);
            }
        }

        let (index, lock) = index::load_component_index(self, channel, release).await?;
        let value = Arc::new(index);
        if let Ok(mut cache) = self.indexes.lock() {
            cache.components.insert(
                key,
                Entry {
                    value: Arc::clone(&value),
                    lock: Some(lock),
                },
            );
        }
        Ok(value)
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
}

/// Whether a cache entry may be served by a fresh read: its lock timestamp
/// must still match the on-disk file's, and that timestamp must pass the
/// freshness window. Anything else falls through to disk/fetch.
fn entry_is_fresh<T>(entry: &Entry<T>, disk_lock: Option<Timestamp>) -> bool {
    entry.lock == disk_lock && disk_lock.is_some_and(|ts| index::lock_is_fresh(&ts))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_index(home: &std::path::Path, body: &str, lock_micros: i64) {
        let downloads = home.join("downloads");
        std::fs::create_dir_all(&downloads).expect("should create downloads dir");
        std::fs::write(downloads.join("index.json"), body).expect("should write index");
        std::fs::write(downloads.join("index.json.lock"), lock_micros.to_string())
            .expect("should write lock");
    }

    #[test]
    fn fresh_read_serves_memory_without_touching_disk() {
        let tempdir = assert_fs::TempDir::new().expect("should create tempdir");
        temp_env::with_var(
            crate::constant::ENVNAME_MOONUP_HOME,
            Some(tempdir.path().as_os_str()),
            || {
                let rt = tokio::runtime::Runtime::new().expect("should create runtime");
                rt.block_on(async {
                    let session = Session::new();
                    write_index(
                        tempdir.path(),
                        r#"{"version": 3, "lastModified": "2026-01-01", "channels": []}"#,
                        jiff::Timestamp::now().as_microsecond(),
                    );

                    // the offline read populates the session cache
                    assert!(session.read_index_offline().await.is_some());

                    // remove the on-disk file: a fresh read must still succeed
                    // by serving the memory entry (the lock timestamp still
                    // matches disk and passes the freshness window)
                    std::fs::remove_file(tempdir.path().join("downloads/index.json"))
                        .expect("should remove index file");
                    assert!(session.read_index().await.is_ok());
                });
            },
        );
    }

    #[test]
    fn stale_offline_entries_are_never_served_as_fresh() {
        let tempdir = assert_fs::TempDir::new().expect("should create tempdir");
        // the dist server points at a closed port, so a fetch fails fast
        // and hermetically
        temp_env::with_var(
            crate::constant::ENVNAME_MOONUP_DIST_SERVER,
            Some("http://127.0.0.1:1"),
            || {
                temp_env::with_var(
                    crate::constant::ENVNAME_MOONUP_HOME,
                    Some(tempdir.path().as_os_str()),
                    || {
                        let rt = tokio::runtime::Runtime::new().expect("should create runtime");
                        rt.block_on(async {
                            let session = Session::new();
                            // a lock timestamp from 2 hours ago: outside the
                            // freshness window
                            let stale = (jiff::Timestamp::now()
                                - jiff::SignedDuration::from_hours(2))
                            .as_microsecond();
                            write_index(
                                tempdir.path(),
                                r#"{"version": 3, "lastModified": "2026-01-01", "channels": []}"#,
                                stale,
                            );

                            // offline mode serves the stale file, best-effort
                            assert!(session.read_index_offline().await.is_some());

                            // fresh mode must not serve the stale entry as
                            // fresh: it attempts a fetch and fails
                            assert!(session.read_index().await.is_err());
                        });
                    },
                );
            },
        );
    }
}

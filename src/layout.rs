//! The resolved locations moonup operates on.
//!
//! A [`Layout`] resolves both home directories once — from the environment at
//! the start of an invocation — and derives every location from those stored
//! values. It is deliberately std-only: the `moonup-shim` binary constructs
//! one in its `main` and threads it through its synchronous flow, so the shim
//! stays a tiny, fast proxy with no session, async, or diagnostics machinery.
//!
//! The CLI constructs one `Layout` per invocation inside the
//! [`Session`](crate::session::Session) and never touches the environment
//! again; tests construct one inside their `temp_env` closure. Because every
//! path flows from a `Layout`, a call site's view of the filesystem is always
//! the one its invocation resolved — and no process-global cache is needed.

use std::path::{Path, PathBuf};

use crate::constant;
use crate::toolchain::ToolchainSpec;

/// The home directories of an invocation, resolved once, plus every path
/// derived from them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    moonup_home: PathBuf,
    moon_home: PathBuf,
}

impl Layout {
    /// Resolve both home directories from the environment.
    pub fn from_env() -> Self {
        Self {
            moonup_home: moonup_home(),
            moon_home: moon_home(),
        }
    }

    /// The moonup home directory this layout resolved.
    pub fn moonup_home(&self) -> &Path {
        &self.moonup_home
    }

    /// The moon home directory this layout resolved.
    pub fn moon_home(&self) -> &Path {
        &self.moon_home
    }

    /// The directory holding installed toolchains, `$MOONUP_HOME/toolchains`.
    pub fn toolchains_dir(&self) -> PathBuf {
        self.moonup_home.join("toolchains")
    }

    /// The install directory of the given toolchain spec,
    /// `$MOONUP_HOME/toolchains/{install name}`.
    pub fn toolchain_dir(&self, spec: &ToolchainSpec) -> PathBuf {
        self.toolchains_dir().join(spec.install_name())
    }

    /// The advisory lock file serializing toolchain mutations,
    /// `$MOONUP_HOME/toolchains/.lock`.
    pub fn toolchains_lock_file(&self) -> PathBuf {
        self.toolchains_dir().join(".lock")
    }

    /// The directory holding cached downloads, `$MOONUP_HOME/downloads`.
    pub fn downloads_dir(&self) -> PathBuf {
        self.moonup_home.join("downloads")
    }

    /// The main index cache file, `$MOONUP_HOME/downloads/index.json`.
    pub fn index_path(&self) -> PathBuf {
        self.downloads_dir().join("index.json")
    }

    /// A channel index cache file,
    /// `$MOONUP_HOME/downloads/channel-{channel}.json`.
    pub fn channel_index_path(&self, channel: &str) -> PathBuf {
        self.downloads_dir().join(format!("channel-{channel}.json"))
    }

    /// A component index cache file,
    /// `$MOONUP_HOME/downloads/{channel}/{key}/{target}.json`.
    pub fn component_index_path(&self, channel: &str, key: &str, target: &str) -> PathBuf {
        self.downloads_dir()
            .join(channel)
            .join(key)
            .join(format!("{}.json", target))
    }

    /// The default-toolchain record, `$MOONUP_HOME/default`.
    pub fn default_toolchain_file(&self) -> PathBuf {
        self.moonup_home.join("default")
    }

    /// The directory moon-published executables are shimmed into,
    /// `$MOON_HOME/bin`.
    pub fn moon_bin_dir(&self) -> PathBuf {
        self.moon_home.join("bin")
    }
}

/// Get MoonBit home directory
///
/// The default MoonBit home directory is `~/.moon`. It may be overridden
/// by the `MOON_HOME` environment variable.
///
/// Prefer [`Layout::from_env`] in invocation code: this convenience resolves
/// from the environment on every call.
///
/// # Returns
///
/// The MoonBit home directory
pub fn moon_home() -> PathBuf {
    if let Some(path) = std::env::var_os(constant::ENVNAME_MOON_HOME) {
        PathBuf::from(path)
    } else {
        dirs::home_dir()
            .map(|path| path.join(constant::MOON_DIR))
            .expect("cannot determine MoonBit home directory")
    }
}

/// Get MoonUp home directory
///
/// The default MoonUp home directory is `~/.moonup`. It may be overridden
/// by the `MOONUP_HOME` environment variable.
///
/// Prefer [`Layout::from_env`] in invocation code: this convenience resolves
/// from the environment on every call.
///
/// # Returns
///
/// The MoonUp home directory
pub fn moonup_home() -> PathBuf {
    if let Some(path) = std::env::var_os(constant::ENVNAME_MOONUP_HOME) {
        PathBuf::from(path)
    } else {
        dirs::home_dir()
            .map(|path| path.join(constant::MOONUP_DIR))
            .expect("cannot determine moonup home directory")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_resolves_once_and_derives_without_the_environment() {
        let tempdir = assert_fs::TempDir::new().expect("should create tempdir");
        temp_env::with_var(
            crate::constant::ENVNAME_MOONUP_HOME,
            Some(tempdir.path().as_os_str()),
            || {
                let layout = Layout::from_env();
                assert_eq!(layout.moonup_home(), tempdir.path());
                assert_eq!(layout.toolchains_dir(), tempdir.path().join("toolchains"));
                assert_eq!(
                    layout.toolchains_lock_file(),
                    tempdir.path().join("toolchains").join(".lock")
                );
                assert_eq!(
                    layout.index_path(),
                    tempdir.path().join("downloads").join("index.json")
                );
                assert_eq!(
                    layout.channel_index_path("nightly"),
                    tempdir
                        .path()
                        .join("downloads")
                        .join("channel-nightly.json")
                );
                assert_eq!(
                    layout.default_toolchain_file(),
                    tempdir.path().join("default")
                );

                // an environment change after resolution is invisible to the
                // layout — the invocation's view is fixed at creation
                let other = assert_fs::TempDir::new().expect("should create tempdir");
                temp_env::with_var(
                    crate::constant::ENVNAME_MOONUP_HOME,
                    Some(other.path().as_os_str()),
                    || {
                        assert_eq!(Layout::from_env().moonup_home(), other.path());
                        assert_eq!(layout.moonup_home(), tempdir.path());
                    },
                );
            },
        );
    }
}

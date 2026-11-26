//! Single source of truth for every path moonup derives from its home
//! directories.
//!
//! All path construction funnels through this module so the same location is
//! never rebuilt in two places. It is deliberately std-only: the
//! `moonup-shim` binary consumes it directly and must stay a tiny, fast,
//! synchronous proxy with no session, async, or diagnostics machinery.
//!
//! Each function re-resolves its home directory from the environment on
//! every call — that is the contract of this layer, and what keeps the
//! `temp_env`-based tests working. The CLI never pays for it: its
//! [`Session`](crate::session::Session) resolves both homes once at creation
//! and derives every location from the stored values through its own
//! accessors, so session-context code is consistent with the session's view
//! and never touches the environment again.

use std::path::PathBuf;

use crate::constant;

/// Get MoonBit home directory
///
/// The default MoonBit home directory is `~/.moon`. It may be overridden
/// by the `MOON_HOME` environment variable.
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

/// The directory holding installed toolchains, `$MOONUP_HOME/toolchains`
pub fn toolchains_dir() -> PathBuf {
    moonup_home().join("toolchains")
}

/// The default-toolchain record, `$MOONUP_HOME/default`
pub fn default_toolchain_file() -> PathBuf {
    moonup_home().join("default")
}

use std::path::PathBuf;

pub mod archive;
pub mod cli;
pub mod constant;
pub mod dist_server;
pub mod fs;
pub(crate) mod reporter;
pub mod toolchain;
pub mod utils;

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

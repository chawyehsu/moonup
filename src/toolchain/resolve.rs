use std::path::PathBuf;

use crate::{constant::TOOLCHAIN_FILE, utils::trimmed_or_none};

/// Iterates over the current directory and all its parent directories to find
/// if there is a [`TOOLCHAIN_FILE`].
///
/// # Returns
///
/// The path to the toolchain file if found
pub fn resolve_toolchain_file() -> Option<PathBuf> {
    let current_dir = std::env::current_dir().ok();
    std::iter::successors(current_dir.as_deref(), |prev| prev.parent())
        .find_map(|dir| Some(dir.join(TOOLCHAIN_FILE)).filter(|p| p.is_file()))
}

/// Detect the pinned version from the current working directory
///
/// # Returns
///
/// The pinned version number if found
pub fn detect_pinned_version() -> Option<String> {
    resolve_toolchain_file()
        .and_then(|path| {
            std::fs::read_to_string(path)
                .map(|s| trimmed_or_none(&s))
                .ok()
        })
        .flatten()
}

/// Detect the default version
///
/// # Returns
///
/// The default version number if found
pub fn detect_default_version() -> Option<String> {
    let default_file = crate::moonup_home().join("default");
    std::fs::read_to_string(default_file)
        .map(|s| trimmed_or_none(&s))
        .ok()
        .flatten()
}

/// Iterates over the current directory and all its parent directories
/// to find if there is a [`TOOLCHAIN_FILE`] and detect the toolchain version.
///
/// # Returns
///
/// The path to actual versioned toolchain
///
/// # Note
///
/// This function is used by the `moonup-shim`, and because we don't want to
/// bloated the shim, miette/tracing should not be used here.
pub fn detect_active_toolchain() -> PathBuf {
    let active = detect_pinned_version()
        .or(detect_default_version())
        .unwrap_or("latest".to_string());
    crate::moonup_home().join("toolchains").join(active)
}

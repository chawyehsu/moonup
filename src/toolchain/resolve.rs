use std::ffi::OsStr;
use std::path::PathBuf;

use crate::{
    constant::{DEFAULT_CHANNEL, TOOLCHAIN_FILE},
    layout::Layout,
    toolchain::ToolchainSpec,
    utils::trimmed_or_none,
};

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

/// Detect the pinned toolchain from the current working directory
///
/// # Returns
///
/// The pinned toolchain version if found
pub fn detect_pinned_toolchain() -> Option<String> {
    resolve_toolchain_file()
        .and_then(|path| {
            std::fs::read_to_string(path)
                .map(|s| trimmed_or_none(&s).map(str::to_owned))
                .ok()
        })
        .flatten()
}

/// Detect the default toolchain
///
/// The default toolchain version is stored in the `$MOONUP_HOME/default` file
///
/// # Returns
///
/// The default toolchain version if found
pub fn detect_default_toolchain(layout: &Layout) -> Option<String> {
    std::fs::read_to_string(layout.default_toolchain_file())
        .map(|s| trimmed_or_none(&s).map(str::to_owned))
        .ok()
        .flatten()
}

/// Iterates over the current directory and all its parent directories
/// to find if there is a [`TOOLCHAIN_FILE`] and detect the toolchain version.
///
/// # Returns
///
/// The detected active toolchain in [`ToolchainSpec`], or a channel-name parse
/// error if a pin/default file contains an invalid spec.
///
/// # Note
///
/// This function is used by the `moonup-shim`; keep it independent of
/// `miette` and `tracing` so the shim stays small.
pub fn detect_active_toolchain(
    layout: &Layout,
) -> Result<ToolchainSpec, super::InvalidChannelName> {
    let active = detect_pinned_toolchain()
        .or(detect_default_toolchain(layout))
        .unwrap_or(DEFAULT_CHANNEL.to_string());
    ToolchainSpec::try_from(active)
}

/// Resolves the executable binary in the given paths.
///
/// # Returns
///
/// The path to the executable binary if found, otherwise `None`.
pub fn resolve_exe<T, U>(binary_name: T, paths: U) -> Option<PathBuf>
where
    T: AsRef<OsStr>,
    U: AsRef<OsStr>,
{
    // Ensure PATHEXT is set on Windows for `which-rs` crate to work correctly
    // NOTE(chawyehsu): This is a workaround for [1], also relevant to [2]
    //
    // [1]: https://github.com/moonbitlang/moonbit-docs/issues/1236
    // [2]: https://github.com/harryfei/which-rs/issues/123
    #[cfg(target_os = "windows")]
    if std::env::var("PATHEXT").is_err() {
        // SAFETY: `set_var` is always safe to call on Windows,
        // whether single-threaded or multi-threaded
        unsafe {
            std::env::set_var("PATHEXT", crate::constant::DEFAULT_PATHEXT);
        };
    }

    which::which_in_global(binary_name, Some(paths))
        .and_then(|mut i| i.next().ok_or(which::Error::CannotFindBinaryPath))
        .ok()
}

/// Resolves a file in the given paths without requiring executable bits.
///
/// # Returns
///
/// The path to the file if found, otherwise `None`.
pub fn resolve_file<T, U>(file_name: T, paths: U) -> Option<PathBuf>
where
    T: AsRef<OsStr>,
    U: AsRef<OsStr>,
{
    let file_name = file_name.as_ref();

    std::env::split_paths(paths.as_ref())
        .map(|path| path.join(file_name))
        .find(|path| path.is_file())
}

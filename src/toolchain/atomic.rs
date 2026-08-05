use miette::{Context, IntoDiagnostic};
use std::path::{Path, PathBuf};

use super::ToolchainSpec;

const STAGING_DIR: &str = ".staging";
const STAGING_SUFFIX: &str = ".new";
const RETIRED_SUFFIX: &str = ".old";
const COMPLETE_SUFFIX: &str = ".complete";

/// The directory where a new toolchain is fully assembled before it is swapped
/// into the live install location.
pub fn staging_dir_for(spec: &ToolchainSpec) -> PathBuf {
    toolchains_root()
        .join(STAGING_DIR)
        .join(format!("{}{}", spec.as_str(), STAGING_SUFFIX))
}

/// The marker written only after a staging directory is fully assembled and
/// verified, distinguishing a complete staging from one cut short by a crash.
pub fn completeness_marker_for(spec: &ToolchainSpec) -> PathBuf {
    toolchains_root()
        .join(STAGING_DIR)
        .join(format!("{}{}", spec.as_str(), COMPLETE_SUFFIX))
}

/// Whether a staging directory is complete enough to be promoted.
pub fn is_complete(spec: &ToolchainSpec) -> bool {
    staging_dir_for(spec).is_dir() && completeness_marker_for(spec).is_file()
}

/// Swap a fully-assembled staging directory into the live install location
/// using two same-volume renames, retiring the previous live directory
/// best-effort.
///
/// A failed swap leaves the live directory untouched and keeps the staging
/// directory in place for a retry, so the currently installed toolchain is
/// never corrupted.
pub fn swap(live: &Path, staging: &Path) -> miette::Result<()> {
    let retired = sibling_with_suffix(staging, STAGING_SUFFIX, RETIRED_SUFFIX);

    // Best-effort clean of a stale retired directory left by a previous swap.
    let _ = crate::fs::remove_dir_all(&retired);

    if live.exists() {
        std::fs::rename(live, &retired)
            .into_diagnostic()
            .wrap_err("toolchain is in use, close programs using it and retry")?;
    }

    std::fs::rename(staging, live)
        .into_diagnostic()
        .wrap_err(format!(
            "failed to move the new toolchain into place at {}",
            live.display()
        ))?;

    let _ = crate::fs::remove_dir_all(&retired);
    let _ = std::fs::remove_file(sibling_with_suffix(
        staging,
        STAGING_SUFFIX,
        COMPLETE_SUFFIX,
    ));
    Ok(())
}

/// Promote a complete staging directory left behind by an interrupted
/// operation when the live toolchain is missing, silently self-healing the
/// crash state (for example a shim-triggered auto-install).
///
/// Returns `Ok(true)` if a staging directory was promoted.
pub fn recover(spec: &ToolchainSpec) -> miette::Result<bool> {
    let live = spec.install_path();
    if !live.exists() && is_complete(spec) {
        swap(&live, &staging_dir_for(spec))?;
        return Ok(true);
    }
    Ok(false)
}

/// Best-effort sweep of any staging leftovers for a toolchain.
pub fn sweep_staging(spec: &ToolchainSpec) {
    let _ = crate::fs::remove_dir_all(staging_dir_for(spec));
    let _ = crate::fs::remove_dir_all(retired_dir_for(spec));
    let _ = std::fs::remove_file(completeness_marker_for(spec));
}

fn toolchains_root() -> PathBuf {
    crate::moonup_home().join("toolchains")
}

fn retired_dir_for(spec: &ToolchainSpec) -> PathBuf {
    sibling_with_suffix(&staging_dir_for(spec), STAGING_SUFFIX, RETIRED_SUFFIX)
}

fn sibling_with_suffix(path: &Path, from: &str, to: &str) -> PathBuf {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    let base = name.strip_suffix(from).unwrap_or(name);
    path.parent()
        .expect("staging path should have a parent")
        .join(format!("{base}{to}"))
}

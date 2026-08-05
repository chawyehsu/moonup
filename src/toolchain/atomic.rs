use miette::{Context, IntoDiagnostic};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::dist_server::schema::Release;

use super::ToolchainSpec;
use super::index::InstallRecipe;

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
///
/// It records the release identity so a staging can be matched against a
/// freshly resolved recipe before it is promoted, and so the post-install
/// steps can be finalized from the promoted release's own metadata.
pub fn completeness_marker_for(spec: &ToolchainSpec) -> PathBuf {
    toolchains_root()
        .join(STAGING_DIR)
        .join(format!("{}{}", spec.as_str(), COMPLETE_SUFFIX))
}

/// Whether a staging directory is complete enough to be promoted.
pub fn is_complete(spec: &ToolchainSpec) -> bool {
    staging_dir_for(spec).is_dir() && completeness_marker_for(spec).is_file()
}

/// The release identity persisted in a completeness marker.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StagedRelease {
    /// The actual release version of the staged toolchain.
    pub version: String,
    /// The build date for nightly releases.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    /// Whether the release bundles a source directory, used by the
    /// post-install steps.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bundle_source_dir: Option<bool>,
}

impl StagedRelease {
    /// Persist the release identity of the recipe being assembled.
    pub fn from_recipe(recipe: &InstallRecipe) -> Self {
        Self {
            version: recipe.release.version.clone(),
            date: recipe.release.date.clone(),
            bundle_source_dir: recipe.release.bundle_source_dir,
        }
    }

    /// Build a recipe carrying the staged release metadata, enough to run the
    /// post-install steps (shims, core library bundle, directory links)
    /// without resolving a fresh release from the network.
    pub fn into_recipe(self, spec: &ToolchainSpec) -> InstallRecipe {
        InstallRecipe {
            spec: spec.clone(),
            release: Release {
                version: self.version,
                layout_version1: None,
                bundle_source_dir: self.bundle_source_dir,
                date: self.date,
                targets: None,
            },
            components: Vec::new(),
        }
    }
}

/// Whether a complete staging directory holds the same release as the given
/// recipe, so a stale staging is never promoted over a newer release.
pub fn staged_matches(spec: &ToolchainSpec, recipe: &InstallRecipe) -> bool {
    let Some(staged) = read_staged_release(spec) else {
        return false;
    };

    match spec {
        ToolchainSpec::Latest | ToolchainSpec::Bleeding => staged.version == recipe.release.version,
        ToolchainSpec::Nightly => staged.date.as_deref() == recipe.release.date.as_deref(),
        ToolchainSpec::Version(v) if v.starts_with("nightly") => {
            staged.date.as_deref() == recipe.release.date.as_deref()
        }
        ToolchainSpec::Version(_) => staged.version == recipe.release.version,
    }
}

/// Swap a fully-assembled staging directory into the live install location
/// using two same-volume renames, retiring the previous live directory
/// best-effort.
///
/// The retired directory is only removed once the new toolchain is in place;
/// if the promotion itself fails, the retired toolchain is restored to the
/// live name so the currently installed toolchain is never lost.
pub fn swap(live: &Path, staging: &Path) -> miette::Result<()> {
    let retired = sibling_with_suffix(staging, STAGING_SUFFIX, RETIRED_SUFFIX);

    // A stale retired directory may linger after a previous swap whose
    // best-effort cleanup failed; remove it only while the live toolchain is
    // intact, never before a promotion that could itself fail.
    if live.exists() && retired.exists() {
        let _ = crate::fs::remove_dir_all(&retired);
    }

    if live.exists() {
        std::fs::rename(live, &retired)
            .into_diagnostic()
            .wrap_err("toolchain is in use, close programs using it and retry")?;
    }

    if let Err(e) = std::fs::rename(staging, live) {
        // Promotion failed; put the retired toolchain back so the live name
        // stays usable and the staging is kept for another retry.
        if !live.exists() && retired.exists() {
            let _ = std::fs::rename(&retired, live);
        }

        let err = std::io::Error::new(
            e.kind(),
            format!(
                "failed to move the new toolchain into place at {}: {e}",
                live.display()
            ),
        );
        return Err(err).into_diagnostic();
    }

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
/// Returns the staged release metadata when a staging directory was promoted,
/// so the caller can finalize the installation (shims, core library, links).
pub fn recover(spec: &ToolchainSpec) -> miette::Result<Option<StagedRelease>> {
    let live = spec.install_path();
    if !live.exists() && is_complete(spec) {
        let staged = read_staged_release(spec).unwrap_or_default();
        swap(&live, &staging_dir_for(spec))?;
        return Ok(Some(staged));
    }
    Ok(None)
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

fn read_staged_release(spec: &ToolchainSpec) -> Option<StagedRelease> {
    std::fs::read_to_string(completeness_marker_for(spec))
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
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

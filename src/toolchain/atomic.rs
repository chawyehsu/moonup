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

/// The directory where the previous live toolchain is parked during a swap
/// before being retired best-effort.
pub fn retired_dir_for(spec: &ToolchainSpec) -> PathBuf {
    sibling_with_suffix(&staging_dir_for(spec), STAGING_SUFFIX, RETIRED_SUFFIX)
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
            requested_spec: spec.clone(),
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
/// live name so the currently installed toolchain is never lost. A stale
/// retired directory that cannot be deleted yet (a process still holds an
/// executable from the previous toolchain, which Windows lets us rename but
/// not delete) is shunted aside under a unique tombstone name so it never
/// blocks this swap; the next swap's best-effort sweep removes it once its
/// processes have exited. The completeness marker is retained as a
/// pending-finalization record and is removed by [`acknowledge`] once the
/// caller has finalized the promotion.
pub fn swap(live: &Path, staging: &Path) -> miette::Result<()> {
    let retired = sibling_with_suffix(staging, STAGING_SUFFIX, RETIRED_SUFFIX);

    if live.exists() {
        // A stale retired directory may linger after a previous swap whose
        // best-effort cleanup failed, typically because a process still holds
        // an executable from the previous toolchain. Sweep it and any
        // tombstones best-effort, then shunt anything still locked aside
        // under a unique name so the deterministic `.old` slot is freed for
        // this swap. This only happens while the live toolchain is intact:
        // never before a promotion that could itself fail, where the retired
        // directory may hold the last usable toolchain.
        sweep_retired(&retired);
        if retired.exists() {
            let tombstone = unique_tombstone(&retired);
            let _ = std::fs::rename(&retired, &tombstone);
            if retired.exists() {
                return Err(miette::miette!(
                    "the previous toolchain is still in use, close programs using it and retry"
                ));
            }
        }
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

    // The new toolchain is in place; retire the previous one best-effort,
    // including tombstones whose locking processes have exited.
    sweep_retired(&retired);
    Ok(())
}

/// Ensure a live toolchain exists for the spec, and return the release
/// metadata needed to finalize it when finalization is still outstanding.
///
/// - If the live toolchain is missing but a complete staging directory exists
///   (an interrupted swap), it is promoted, silently self-healing the crash
///   state (for example a shim-triggered auto-install). A corrupt completeness
///   marker cannot drive finalization reliably, so such a staging is not
///   promoted.
/// - If the live toolchain is already in place but its finalization never
///   completed, the pending marker is returned so the caller can retry the
///   post-install steps.
///
/// Returns `Ok(Some(..))` in both cases, `Ok(None)` otherwise.
pub fn recover(spec: &ToolchainSpec) -> miette::Result<Option<StagedRelease>> {
    let live = spec.install_path();
    let staging = staging_dir_for(spec);
    let marker = completeness_marker_for(spec);

    if !live.exists() && is_complete(spec) {
        let Some(staged) = read_staged_release(spec) else {
            return Ok(None);
        };
        swap(&live, &staging)?;
        return Ok(Some(staged));
    }

    if live.exists()
        && !staging.exists()
        && marker.is_file()
        && let Some(staged) = read_staged_release(spec)
    {
        return Ok(Some(staged));
    }

    Ok(None)
}

/// Acknowledge successful finalization of a promoted toolchain by removing its
/// pending-finalization marker.
pub fn acknowledge(spec: &ToolchainSpec) {
    let _ = std::fs::remove_file(completeness_marker_for(spec));
}

/// Best-effort sweep of any staging leftovers for a toolchain.
pub fn sweep_staging(spec: &ToolchainSpec) {
    let _ = crate::fs::remove_dir_all(staging_dir_for(spec));
    sweep_retired(&retired_dir_for(spec));
    let _ = std::fs::remove_file(completeness_marker_for(spec));
}

/// Best-effort removal of a retired directory and its shunted tombstones,
/// matching `<name>.old` and `<name>.old.<pid>.<counter>` siblings. The
/// literal dot keeps a spec whose name prefixes another (e.g. `nightly` vs
/// `nightly-2025-01-01`) from sweeping the other's artifacts.
fn sweep_retired(retired: &Path) {
    let Some(parent) = retired.parent() else {
        return;
    };
    let Some(name) = retired.file_name().and_then(|n| n.to_str()) else {
        return;
    };

    let Ok(read_dir) = std::fs::read_dir(parent) else {
        return;
    };
    for entry in read_dir.flatten() {
        let Some(entry_name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if (entry_name == name || entry_name.starts_with(&format!("{name}.")))
            && entry.file_type().map(|t| t.is_dir()).unwrap_or(false)
        {
            let _ = crate::fs::remove_dir_all(entry.path());
        }
    }
}

/// A unique tombstone path derived from a retired directory that cannot be
/// deleted yet, `<name>.old.<pid>.<counter>`, following the retirement naming
/// convention of `replace_exe`. The process id disambiguates concurrent
/// moonup processes, the counter disambiguates retries within one.
fn unique_tombstone(retired: &Path) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let parent = retired.parent().unwrap_or_else(|| Path::new(""));
    let name = retired
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();

    let pid = std::process::id();
    let mut counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    loop {
        let candidate = parent.join(format!("{name}.{pid}.{counter}"));
        if !candidate.exists() {
            return candidate;
        }
        counter += 1;
    }
}

fn toolchains_root() -> PathBuf {
    crate::moonup_home().join("toolchains")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_tombstone_name_follows_convention() {
        let tempdir = assert_fs::TempDir::new().expect("should create tempdir");
        let retired = tempdir.path().join("latest.old");

        let tombstone = unique_tombstone(&retired);
        let name = tombstone
            .file_name()
            .expect("should have a file name")
            .to_string_lossy();

        assert!(
            name.starts_with("latest.old."),
            "tombstone should follow the retirement naming convention, got {name}"
        );
        let suffix = name.strip_prefix("latest.old.").expect("prefix checked");
        let (pid, counter) = suffix.split_once('.').expect("pid.counter suffix");
        assert_eq!(pid, std::process::id().to_string());
        assert!(counter.parse::<u64>().is_ok());
    }

    #[test]
    fn unique_tombstone_skips_existing_names() {
        let tempdir = assert_fs::TempDir::new().expect("should create tempdir");
        let retired = tempdir.path().join("latest.old");

        let first = unique_tombstone(&retired);
        std::fs::create_dir_all(&first).expect("should create first tombstone");

        let second = unique_tombstone(&retired);
        assert_ne!(first, second, "a collision should bump the counter");
        assert!(
            !second.exists(),
            "the next tombstone should not already exist"
        );
    }

    #[test]
    fn sweep_retired_distinguishes_prefixing_specs() {
        let tempdir = assert_fs::TempDir::new().expect("should create tempdir");
        let staging = tempdir.path();

        // the `nightly` spec's own retired dir and tombstone
        std::fs::create_dir_all(staging.join("nightly.old")).expect("should create stale old");
        std::fs::create_dir_all(staging.join("nightly.old.1.1")).expect("should create tombstone");
        // a versioned nightly spec whose name is prefixed by `nightly`
        std::fs::create_dir_all(staging.join("nightly-2025-01-01.old"))
            .expect("should create versioned old");

        sweep_retired(&staging.join("nightly.old"));

        assert!(!staging.join("nightly.old").exists());
        assert!(!staging.join("nightly.old.1.1").exists());
        assert!(
            staging.join("nightly-2025-01-01.old").exists(),
            "a spec whose name prefixes another must not sweep the other's artifacts"
        );
    }
}

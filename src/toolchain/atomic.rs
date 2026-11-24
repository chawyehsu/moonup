use miette::{Context, IntoDiagnostic};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::install::InstallRecipe;
use super::{Finalization, InstallMeta, InstalledToolchain, ToolchainSpec};

pub const STAGING_DIR: &str = ".staging";
pub const STAGING_SUFFIX: &str = ".new";
const RETIRED_SUFFIX: &str = ".old";
/// The completeness marker written by older moonup versions, swept as a
/// staging leftover.
const LEGACY_COMPLETE_SUFFIX: &str = ".complete";

/// The directory where a new toolchain is fully assembled before it is swapped
/// into the live install location.
pub fn staging_dir_for(spec: &ToolchainSpec) -> PathBuf {
    toolchains_root()
        .join(STAGING_DIR)
        .join(format!("{}{}", spec.install_name(), STAGING_SUFFIX))
}

/// Whether a complete staging directory holds the same release as the given
/// recipe, so a stale staging is never promoted over a newer release.
///
/// A staging is complete when it holds a valid install record whose identity
/// matches the spec and whose finalization payload is present; stagings left
/// by older moonup versions carry no payload and count as stale. Returns an
/// error if the staged record belongs to a different spec; returns `false`
/// when the staging is absent, incomplete, or records another release.
pub fn staged_matches(spec: &ToolchainSpec, recipe: &InstallRecipe) -> miette::Result<bool> {
    let Some(record) = read_staged_record(spec) else {
        return Ok(false);
    };

    ensure_record_spec(spec, &record)?;
    Ok(record.finalization.is_some() && record.key == recipe.release.key())
}

/// Verify that the live install path is either vacant or belongs to this spec.
///
/// An existing path whose identity cannot be read is rejected rather than
/// replaced, since it may be a different toolchain sharing the same flat name.
pub(crate) fn ensure_install_path_available(spec: &ToolchainSpec) -> miette::Result<()> {
    let live = spec.install_path();
    if !live.exists() {
        return Ok(());
    }

    let installed = InstalledToolchain::from_path(&live).map_err(|e| {
        miette::miette!(
            "path '{}' is occupied and cannot be verified ({e})",
            live.display()
        )
    })?;

    if installed.spec != *spec {
        return Err(miette::miette!(
            "cannot install '{}' to path '{}' occupied by '{}', uninstall the existing toolchain first",
            spec,
            live.display(),
            installed.spec,
        ));
    }

    Ok(())
}

/// Swap a fully-assembled staging directory into the live install location
/// using two same-volume renames, retiring the previous live directory
/// best-effort.
///
/// The live install path must be vacant or belong to this spec, and only a
/// staging whose own install record proves its identity is promoted. The
/// retired directory is only removed once the new toolchain is in place; if
/// the promotion itself fails, the retired toolchain is restored to the live
/// name so the currently installed toolchain is never lost. Retired
/// directories are parked under unique names (`<name>.old.<pid>.<n>`,
/// mirroring the shim retirement convention), so a leftover that cannot be
/// deleted yet (a process still holds an executable from the previous
/// toolchain, which Windows lets us rename but not delete) never blocks the
/// next swap; later swaps sweep such leftovers best-effort once their
/// processes have exited.
pub fn swap(spec: &ToolchainSpec) -> miette::Result<()> {
    let live = spec.install_path();
    let staging = staging_dir_for(spec);

    ensure_install_path_available(spec)?;

    // Only promote a staging whose own install record proves it belongs to
    // this spec; an unverifiable staging is never promoted.
    if staging.exists() {
        let Some(record) = read_staged_record(spec) else {
            return Err(miette::miette!(
                "refusing to promote staging for '{}': its toolchain identity cannot be verified",
                spec
            ));
        };
        ensure_record_spec(spec, &record)?;
    }

    swap_paths(&live, &staging)
}

fn swap_paths(live: &Path, staging: &Path) -> miette::Result<()> {
    let staging_parent = staging.parent().expect("staging path should have a parent");
    let base = retired_base(staging);
    let retired = unique_retired_path(staging_parent, &base);

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

    // The new toolchain is in place; sweep this spec's retired leftovers
    // best-effort, including ones whose locking processes have since exited.
    sweep_leftovers(&base, staging_parent);
    Ok(())
}

/// Ensure a live toolchain exists for the spec, and return the finalization
/// inputs needed to finalize it when finalization is still outstanding.
///
/// - If the live toolchain is missing but a complete staging directory exists
///   (an interrupted swap), it is promoted, silently self-healing the crash
///   state (for example a shim-triggered auto-install). A staging whose
///   record carries no finalization payload — a partial extraction, or one
///   left by an older moonup version — is not promoted; the next install
///   discards and re-downloads it.
/// - If the live toolchain is already in place but its finalization never
///   completed, the pending finalization inputs are returned so the caller
///   can retry the post-install steps.
///
/// Returns `Ok(Some(..))` in both cases, `Ok(None)` otherwise.
pub fn recover(spec: &ToolchainSpec) -> miette::Result<Option<Finalization>> {
    let live = spec.install_path();
    let staging = staging_dir_for(spec);

    if !live.exists() {
        let Some(record) = read_staged_record(spec) else {
            return Ok(None);
        };
        ensure_record_spec(spec, &record)?;
        let Some(finalization) = record.finalization else {
            return Ok(None);
        };
        swap(spec)?;
        return Ok(Some(finalization));
    }

    if !staging.exists()
        && let Ok(record) = super::read_install_meta(&live)
        && let Some(finalization) = record.finalization
    {
        ensure_install_path_available(spec)?;
        return Ok(Some(finalization));
    }

    Ok(None)
}

/// Ensure a staging record belongs to the given spec.
///
/// Install names are flat, so a floating channel and a pin can map to the
/// same directory; a record whose identity differs belongs to another
/// toolchain and is refused rather than promoted or finalized.
fn ensure_record_spec(spec: &ToolchainSpec, record: &InstallMeta) -> miette::Result<()> {
    if record.channel != *spec.channel()
        || record.floating != matches!(spec, ToolchainSpec::Channel(_))
    {
        return Err(miette::miette!(
            "interrupted install for '{}' contains a different toolchain; refusing recovery",
            spec
        ));
    }
    Ok(())
}

/// Acknowledge successful finalization of a promoted toolchain by rewriting
/// its install record without the finalization payload.
///
/// Best-effort: a failed rewrite leaves the payload in place, and the next
/// install or update simply retries the post-install steps.
pub fn acknowledge(spec: &ToolchainSpec) {
    let live = spec.install_path();
    let Ok(mut record) = super::read_install_meta(&live) else {
        return;
    };
    if record.finalization.take().is_none() {
        return;
    }
    let _ = record.save(&live);
}

/// Best-effort sweep of any staging leftovers for a toolchain: the staging
/// directory itself, retired directories from earlier swaps, and legacy
/// artifacts left by older moonup versions.
pub fn sweep_staging(spec: &ToolchainSpec) {
    let staging = staging_dir_for(spec);
    let _ = crate::fs::remove_dir_all(&staging);
    let parent = staging.parent().expect("staging path should have a parent");
    sweep_leftovers(&retired_base(&staging), parent);
}

/// Best-effort removal of a spec's retired directories and legacy staging
/// artifacts, matching `<name>.old`, `<name>.old.<pid>.<counter>` and the
/// legacy `<name>.complete` marker. The literal dot after `<name>` keeps a
/// spec whose name prefixes another (e.g. `nightly` vs `nightly-2025-01-01`)
/// from sweeping the other's artifacts.
fn sweep_leftovers(base: &str, parent: &Path) {
    let retired = format!("{base}{RETIRED_SUFFIX}");
    let legacy_marker = format!("{base}{LEGACY_COMPLETE_SUFFIX}");

    let Ok(read_dir) = std::fs::read_dir(parent) else {
        return;
    };
    for entry in read_dir.flatten() {
        let Some(entry_name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let is_retired = entry_name == retired || entry_name.starts_with(&format!("{retired}."));
        let is_legacy_marker = entry_name == legacy_marker;
        if !is_retired && !is_legacy_marker {
            continue;
        }
        match entry.file_type() {
            Ok(t) if t.is_dir() => {
                let _ = crate::fs::remove_dir_all(entry.path());
            }
            Ok(t) if t.is_file() => {
                let _ = std::fs::remove_file(entry.path());
            }
            _ => {}
        }
    }
}

/// The `<name>` part shared by a spec's staging path and its leftover
/// artifact names: the staging directory's file name without the `.new`
/// suffix, i.e. the spec's install name.
fn retired_base(staging: &Path) -> String {
    staging
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.strip_suffix(STAGING_SUFFIX).unwrap_or(n).to_owned())
        .unwrap_or_default()
}

/// A unique retired-directory path for one swap,
/// `<name>.old.<pid>.<counter>`, following the retirement naming convention
/// of `replace_exe`. The process id disambiguates concurrent moonup
/// processes, the counter disambiguates swaps within one process, and the
/// `exists` loop guards against a pid-counter collision with a leftover that
/// cannot be deleted yet.
fn unique_retired_path(parent: &Path, base: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let pid = std::process::id();
    let mut counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    loop {
        let candidate = parent.join(format!("{base}{RETIRED_SUFFIX}.{pid}.{counter}"));
        if !candidate.exists() {
            return candidate;
        }
        counter += 1;
    }
}

fn toolchains_root() -> PathBuf {
    crate::moonup_home().join("toolchains")
}

/// Read the install record inside a staging directory, `None` when the
/// staging is absent or its record is missing or unreadable.
fn read_staged_record(spec: &ToolchainSpec) -> Option<InstallMeta> {
    let staging = staging_dir_for(spec);
    if !staging.is_dir() {
        return None;
    }
    super::read_install_meta(&staging).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dist_server::schema::ChannelName;

    fn channel_name(value: &str) -> ChannelName {
        ChannelName::parse(value).expect("test channel name should be valid")
    }

    #[test]
    fn unique_retired_path_follows_convention_and_skips_existing() {
        let tempdir = assert_fs::TempDir::new().expect("should create tempdir");
        let base = "latest";

        let first = unique_retired_path(tempdir.path(), base);
        let name = first
            .file_name()
            .expect("should have a file name")
            .to_string_lossy();

        assert!(
            name.starts_with("latest.old."),
            "retired path should follow the retirement naming convention, got {name}"
        );
        let suffix = name.strip_prefix("latest.old.").expect("prefix checked");
        let (pid, counter) = suffix.split_once('.').expect("pid.counter suffix");
        assert_eq!(pid, std::process::id().to_string());
        assert!(counter.parse::<u64>().is_ok());

        std::fs::create_dir_all(&first).expect("should create the first retired dir");
        let second = unique_retired_path(tempdir.path(), base);
        assert_ne!(first, second, "a collision should bump the counter");
        assert!(
            !second.exists(),
            "the next retired path should not already exist"
        );
    }

    #[test]
    fn staged_record_identity_distinguishes_a_channel_from_a_colliding_pin() {
        let staged = InstallMeta {
            channel: channel_name("lts-2026-01-01"),
            key: "2026-01-01".to_owned(),
            floating: true,
            finalization: None,
        };
        let colliding_pin = ToolchainSpec::try_from("lts@2026-01-01").expect("valid pin");
        let floating_channel =
            ToolchainSpec::try_from("lts-2026-01-01").expect("valid channel name");

        assert!(ensure_record_spec(&colliding_pin, &staged).is_err());
        assert!(ensure_record_spec(&floating_channel, &staged).is_ok());
    }

    #[test]
    fn sweep_leftovers_distinguishes_prefixing_specs() {
        let tempdir = assert_fs::TempDir::new().expect("should create tempdir");

        // the `nightly` spec's own retired dirs and legacy marker
        std::fs::create_dir_all(tempdir.path().join("nightly.old"))
            .expect("should create legacy old");
        std::fs::create_dir_all(tempdir.path().join("nightly.old.1.1"))
            .expect("should create unique old");
        std::fs::write(tempdir.path().join("nightly.complete"), "{}")
            .expect("should create legacy marker");
        // a versioned nightly spec whose name is prefixed by `nightly`
        std::fs::create_dir_all(tempdir.path().join("nightly-2025-01-01.old"))
            .expect("should create versioned old");

        sweep_leftovers("nightly", tempdir.path());

        assert!(!tempdir.path().join("nightly.old").exists());
        assert!(!tempdir.path().join("nightly.old.1.1").exists());
        assert!(!tempdir.path().join("nightly.complete").exists());
        assert!(
            tempdir.path().join("nightly-2025-01-01.old").exists(),
            "a spec whose name prefixes another must not sweep the other's artifacts"
        );
    }
}

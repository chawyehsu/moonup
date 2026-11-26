use std::path::PathBuf;

use moonup::{
    constant,
    dist_server::schema::Release,
    toolchain::{ToolchainSpec, atomic, install::InstallRecipe, installed_toolchains},
};

use crate::util::crash_state as cs;

fn layout() -> moonup::layout::Layout {
    moonup::layout::Layout::from_env()
}

fn spec(value: &str) -> ToolchainSpec {
    ToolchainSpec::try_from(value).expect("test spec should be valid")
}

fn swap_paths(spec: &ToolchainSpec) -> (PathBuf, PathBuf) {
    (
        layout().toolchain_dir(spec),
        atomic::staging_dir_for(&layout(), spec),
    )
}

fn recipe(spec: &ToolchainSpec, version: &str) -> InstallRecipe {
    InstallRecipe {
        spec: spec.clone(),
        requested_spec: spec.clone(),
        release: Release::new(version),
        components: Vec::new(),
    }
}

/// A finalization payload as `populate` records it for the mock releases.
fn finalization() -> serde_json::Value {
    serde_json::json!({"bundle_source_dir": false, "moonx": "available"})
}

fn live_moon(spec: &ToolchainSpec) -> String {
    std::fs::read_to_string(layout().toolchain_dir(spec).join("bin").join("moon"))
        .expect("should read live moon")
}

#[test]
fn test_recover_promotes_complete_staging() {
    let tempdir = assert_fs::TempDir::new().expect("should create tempdir");
    let moonup_home = tempdir.path().join(".moonup");

    temp_env::with_var(
        constant::ENVNAME_MOONUP_HOME,
        Some(moonup_home.as_os_str()),
        || {
            let spec = spec("latest");
            let live_dir = layout().toolchain_dir(&spec);

            // simulate a crash that left a fully assembled staging directory
            cs::write_complete_staging(&spec, "0.1.0", finalization());

            assert!(!live_dir.exists());

            let staged = atomic::recover(&layout(), &spec)
                .expect("recovery should not fail")
                .expect("complete staging should be promoted");
            assert_eq!(staged.bundle_source_dir, Some(false));

            assert!(
                live_dir.join("bin").join("moon").exists(),
                "staging should be promoted to the live location"
            );
            assert!(
                !atomic::staging_dir_for(&layout(), &spec).exists(),
                "staging dir should be consumed by the swap"
            );
            assert!(
                cs::live_record_has_finalization(&spec),
                "the promoted record should still carry the finalization payload"
            );

            atomic::acknowledge(&layout(), &spec);
            assert!(
                !cs::live_record_has_finalization(&spec),
                "acknowledgment should clear the finalization payload"
            );
        },
    );
}

#[test]
fn test_recover_does_not_promote_incomplete_staging() {
    let tempdir = assert_fs::TempDir::new().expect("should create tempdir");
    let moonup_home = tempdir.path().join(".moonup");

    temp_env::with_var(
        constant::ENVNAME_MOONUP_HOME,
        Some(moonup_home.as_os_str()),
        || {
            let spec = spec("latest");

            // a staging dir without an install record was cut short
            cs::write_partial_staging(&spec);

            let staged = atomic::recover(&layout(), &spec).expect("recovery should not fail");
            assert!(staged.is_none(), "partial staging should not be promoted");
            assert!(
                !layout().toolchain_dir(&spec).exists(),
                "no live toolchain should exist"
            );
        },
    );
}

#[test]
fn test_recover_retries_pending_finalization() {
    let tempdir = assert_fs::TempDir::new().expect("should create tempdir");
    let moonup_home = tempdir.path().join(".moonup");

    temp_env::with_var(
        constant::ENVNAME_MOONUP_HOME,
        Some(moonup_home.as_os_str()),
        || {
            let spec = spec("latest");
            let live_dir = layout().toolchain_dir(&spec);

            // the live toolchain is already promoted but its finalization
            // never completed: its install record still carries the payload
            cs::write_live_install(
                &spec,
                &cs::install_record(&spec, "0.1.0", Some(finalization())),
            );

            let staged = atomic::recover(&layout(), &spec).expect("recovery should not fail");
            assert!(staged.is_some(), "pending finalization should be retried");

            assert!(
                live_dir.join("bin").join("moon").exists(),
                "live toolchain should be left untouched"
            );
        },
    );
}

#[test]
fn test_recover_refuses_live_install_with_a_different_identity() {
    let tempdir = assert_fs::TempDir::new().expect("should create tempdir");
    let moonup_home = tempdir.path().join(".moonup");

    temp_env::with_var(
        constant::ENVNAME_MOONUP_HOME,
        Some(moonup_home.as_os_str()),
        || {
            let pin = spec("lts@2026-01-01");
            let colliding_channel = spec("lts-2026-01-01");

            // the flat install name is held by the colliding channel's
            // toolchain, whose record carries a pending finalization
            cs::write_live_install(
                &colliding_channel,
                &cs::install_record(&colliding_channel, "2026-01-01", Some(finalization())),
            );

            let error = atomic::recover(&layout(), &pin)
                .expect_err("recovery must not finalize another toolchain's live path");
            assert!(
                error.to_string().contains("occupied by"),
                "unexpected recovery error: {error}"
            );
            assert!(
                layout().toolchain_dir(&colliding_channel).exists(),
                "the existing install must be preserved"
            );
        },
    );
}

#[test]
fn test_swap_promotes_and_retires() {
    let tempdir = assert_fs::TempDir::new().expect("should create tempdir");
    let moonup_home = tempdir.path().join(".moonup");

    temp_env::with_var(
        constant::ENVNAME_MOONUP_HOME,
        Some(moonup_home.as_os_str()),
        || {
            let spec = spec("latest");

            cs::write_live_install(&spec, &cs::install_record(&spec, "0.1.0", None));
            cs::write_complete_staging(&spec, "0.1.0", finalization());

            atomic::swap(&layout(), &spec).expect("swap should succeed");

            assert_eq!(live_moon(&spec), "new");
            assert!(
                !atomic::staging_dir_for(&layout(), &spec).exists(),
                "staging should be consumed"
            );
            assert!(
                !cs::has_retired_leftover(&spec),
                "retired dir should be removed"
            );
            assert!(
                cs::live_record_has_finalization(&spec),
                "the record should survive the swap as a pending-finalization marker"
            );

            atomic::acknowledge(&layout(), &spec);
            assert!(
                !cs::live_record_has_finalization(&spec),
                "acknowledgment should clear the finalization payload"
            );
        },
    );
}

#[test]
fn test_swap_refuses_a_colliding_live_install() {
    let tempdir = assert_fs::TempDir::new().expect("should create tempdir");
    let moonup_home = tempdir.path().join(".moonup");

    temp_env::with_var(
        constant::ENVNAME_MOONUP_HOME,
        Some(moonup_home.as_os_str()),
        || {
            let pin = spec("lts@2026-01-01");
            let colliding_channel = spec("lts-2026-01-01");

            cs::write_live_install(
                &colliding_channel,
                &cs::install_record(&colliding_channel, "2026-01-01", None),
            );
            cs::write_complete_staging(&pin, "2026-01-01", finalization());

            assert!(atomic::swap(&layout(), &pin).is_err());
            assert!(
                layout().toolchain_dir(&colliding_channel).exists(),
                "the existing install must be preserved"
            );
            assert!(
                atomic::staging_dir_for(&layout(), &pin).exists(),
                "the staged pin must be preserved"
            );

            std::fs::remove_file(layout().toolchain_dir(&colliding_channel).join("version"))
                .expect("should remove live record");
            assert!(atomic::swap(&layout(), &pin).is_err());
            assert!(
                layout().toolchain_dir(&colliding_channel).exists(),
                "the unidentifiable path must be preserved"
            );
        },
    );
}

#[test]
fn test_swap_restores_live_when_staging_is_missing() {
    let tempdir = assert_fs::TempDir::new().expect("should create tempdir");
    let moonup_home = tempdir.path().join(".moonup");

    temp_env::with_var(
        constant::ENVNAME_MOONUP_HOME,
        Some(moonup_home.as_os_str()),
        || {
            let spec = spec("latest");
            let (live_dir, _) = swap_paths(&spec);

            std::fs::create_dir_all(cs::staging_root()).expect("should create staging parent");
            cs::write_live_install(&spec, &cs::install_record(&spec, "0.1.0", None));

            // no staging at all: the promotion fails and the retired live
            // toolchain is restored to the live name
            let result = atomic::swap(&layout(), &spec);
            assert!(
                result.is_err(),
                "promotion of a missing staging should fail"
            );

            assert!(
                live_dir.join("bin").join("moon").exists(),
                "retired toolchain should be restored to the live name"
            );
            assert_eq!(live_moon(&spec), "live");
            assert!(
                !cs::has_retired_leftover(&spec),
                "retired dir should be consumed by the restore"
            );
        },
    );
}

#[test]
fn test_staged_matches_requires_same_release() {
    let tempdir = assert_fs::TempDir::new().expect("should create tempdir");
    let moonup_home = tempdir.path().join(".moonup");

    temp_env::with_var(
        constant::ENVNAME_MOONUP_HOME,
        Some(moonup_home.as_os_str()),
        || {
            let spec = spec("latest");
            cs::write_complete_staging(&spec, "0.1.0", finalization());

            assert!(
                atomic::staged_matches(&layout(), &spec, &recipe(&spec, "0.1.0"))
                    .expect("matching spec should be accepted"),
                "matching release should be promoted"
            );
            assert!(
                !atomic::staged_matches(&layout(), &spec, &recipe(&spec, "0.2.0"))
                    .expect("same spec with stale release should be accepted for cleanup"),
                "a stale staging must not be promoted over a newer release"
            );

            // a staging whose record carries no finalization payload — as an
            // older moonup left behind — is stale, never promotable
            std::fs::write(
                atomic::staging_dir_for(&layout(), &spec).join("version"),
                cs::install_record(&spec, "0.1.0", None),
            )
            .expect("should rewrite staged record");
            assert!(
                !atomic::staged_matches(&layout(), &spec, &recipe(&spec, "0.1.0"))
                    .expect("should read staged record"),
                "a staging without finalization inputs must not be promoted"
            );

            // a record for a different spec is reported, not treated as an
            // ordinary stale stage
            let pin = ToolchainSpec::try_from("lts@2026-01-01").expect("qualified pin is valid");
            let colliding_channel =
                ToolchainSpec::try_from("lts-2026-01-01").expect("channel name is valid");
            cs::write_complete_staging(&pin, "2026-01-01", finalization());
            std::fs::write(
                atomic::staging_dir_for(&layout(), &pin).join("version"),
                cs::install_record(&colliding_channel, "2026-01-01", Some(finalization())),
            )
            .expect("should rewrite staged record as foreign");
            assert!(
                atomic::staged_matches(&layout(), &pin, &recipe(&pin, "2026-01-01")).is_err(),
                "a stage for a different spec must be reported, not treated as an ordinary stale stage"
            );
        },
    );
}

#[test]
fn test_installed_toolchains_excludes_staging() {
    let tempdir = assert_fs::TempDir::new().expect("should create tempdir");
    let moonup_home = tempdir.path().join(".moonup");

    temp_env::with_var(
        constant::ENVNAME_MOONUP_HOME,
        Some(moonup_home.as_os_str()),
        || {
            cs::write_live_install(&spec("latest"), "0.1.0\n");
            cs::write_partial_staging(&spec("latest"));

            let installed = installed_toolchains(&moonup::session::Session::new())
                .expect("should list installed toolchains");
            assert_eq!(installed.len(), 1);
            assert_eq!(installed[0].spec, spec("latest"));
        },
    );
}

#[test]
fn test_installed_toolchains_stale_latest_cache_falls_back_to_numeric() {
    let tempdir = assert_fs::TempDir::new().expect("should create tempdir");
    let moonup_home = tempdir.path().join(".moonup");

    temp_env::with_var(
        constant::ENVNAME_MOONUP_HOME,
        Some(moonup_home.as_os_str()),
        || {
            // A stale cache that no longer lists the installed `0.11.0` must not
            // sort it below `0.10.0` (which the cache does list); the whole
            // latest group falls back to numeric version ordering.
            let toolchains_dir = moonup_home.join("toolchains");
            for v in ["0.10.0", "0.11.0"] {
                std::fs::create_dir_all(toolchains_dir.join(v))
                    .expect("should create toolchain dir");
            }

            let downloads_dir = moonup_home.join("downloads");
            std::fs::create_dir_all(&downloads_dir).expect("should create downloads dir");
            std::fs::write(
                downloads_dir.join("channel-latest.json"),
                r#"{ "version": 3, "lastModified": "STALE", "releases": [ { "version": "0.10.0", "targets": ["aarch64-apple-darwin"] } ] }"#,
            )
            .expect("should write stale latest cache");

            let installed = installed_toolchains(&moonup::session::Session::new())
                .expect("should list installed toolchains");
            let names = installed
                .iter()
                .map(|t| t.spec.to_string())
                .collect::<Vec<_>>();
            assert_eq!(names, vec!["0.10.0", "0.11.0"]);
        },
    );
}

#[test]
fn test_swap_sweeps_retired_leftovers() {
    let tempdir = assert_fs::TempDir::new().expect("should create tempdir");
    let moonup_home = tempdir.path().join(".moonup");

    temp_env::with_var(
        constant::ENVNAME_MOONUP_HOME,
        Some(moonup_home.as_os_str()),
        || {
            let spec = spec("latest");

            // leftovers from earlier swaps and from older moonup versions
            let legacy_retired = cs::write_legacy_retired(&spec);
            let unique_retired = cs::write_unique_retired(&spec, 0);
            let legacy_marker = cs::write_legacy_complete_marker(&spec, "0.1.0");

            cs::write_live_install(&spec, &cs::install_record(&spec, "0.1.0", None));
            cs::write_complete_staging(&spec, "0.1.0", finalization());

            atomic::swap(&layout(), &spec).expect("swap should succeed");

            assert_eq!(live_moon(&spec), "new");
            assert!(
                !atomic::staging_dir_for(&layout(), &spec).exists(),
                "staging should be consumed by the swap"
            );
            assert!(
                !legacy_retired.exists(),
                "legacy retired dir should be swept"
            );
            assert!(
                !unique_retired.exists(),
                "retired leftover from an earlier swap should be swept"
            );
            assert!(
                !legacy_marker.exists(),
                "legacy completeness marker should be swept"
            );
            assert!(
                !cs::has_retired_leftover(&spec),
                "no retired leftover should remain"
            );
        },
    );
}

#[test]
fn test_sweep_staging_is_scoped_to_spec() {
    let tempdir = assert_fs::TempDir::new().expect("should create tempdir");
    let moonup_home = tempdir.path().join(".moonup");

    temp_env::with_var(
        constant::ENVNAME_MOONUP_HOME,
        Some(moonup_home.as_os_str()),
        || {
            let spec = spec("latest");

            // a versioned nightly spec the `latest` sweep must not touch
            let nightly_spec = ToolchainSpec::try_from("nightly-2025-01-01")
                .expect("versioned nightly spec is valid");
            let nightly_old = cs::write_legacy_retired(&nightly_spec);

            let staging_dir = cs::write_partial_staging(&spec);
            let latest_old = cs::write_legacy_retired(&spec);
            let latest_tombstone = cs::write_unique_retired(&spec, 0);
            let latest_marker = cs::write_legacy_complete_marker(&spec, "0.1.0");

            atomic::sweep_staging(&layout(), &spec);

            assert!(!latest_old.exists(), "latest.old should be swept");
            assert!(
                !staging_dir.exists(),
                "latest.new staging dir should be swept"
            );
            assert!(
                !latest_tombstone.exists(),
                "latest retired leftovers should be swept"
            );
            assert!(
                !latest_marker.exists(),
                "latest legacy marker should be swept"
            );
            assert!(
                nightly_old.exists(),
                "another spec's retired dir must not be swept"
            );
        },
    );
}

#[cfg(target_os = "windows")]
#[test]
fn test_swap_tolerates_locked_retired_leftover() {
    use std::process::Command;
    use std::time::Duration;

    // Child mode: keep a real running image mapped inside a retired leftover
    // dir so the parent exercises Windows' rename-ok / delete-denied
    // semantics. The parent selects this mode with `MOONUP_TEST_HOLD`.
    if std::env::var_os("MOONUP_TEST_HOLD").is_some() {
        std::thread::sleep(Duration::from_secs(60));
        return;
    }

    let tempdir = assert_fs::TempDir::new().expect("should create tempdir");
    let moonup_home = tempdir.path().join(".moonup");

    temp_env::with_var(
        constant::ENVNAME_MOONUP_HOME,
        Some(moonup_home.as_os_str()),
        || {
            let spec = spec("latest");

            // a retired leftover from an earlier swap holds a real running
            // image: it can be renamed but not deleted
            let leftover = cs::write_unique_retired(&spec, 0);
            std::fs::create_dir_all(leftover.join("bin")).expect("should create leftover bin dir");
            let exe = std::env::current_exe().expect("should resolve current exe");
            std::fs::copy(&exe, leftover.join("bin").join("holder.exe"))
                .expect("should copy current exe");
            let mut child = Command::new(leftover.join("bin").join("holder.exe"))
                .arg("test_swap_tolerates_locked_retired_leftover")
                .env("MOONUP_TEST_HOLD", "1")
                .spawn()
                .expect("should spawn holder child");
            std::thread::sleep(Duration::from_millis(500));

            cs::write_live_install(&spec, &cs::install_record(&spec, "0.1.0", None));
            cs::write_complete_staging(&spec, "0.1.0", finalization());

            atomic::swap(&layout(), &spec).expect("swap should tolerate a locked retired leftover");

            assert_eq!(live_moon(&spec), "new");
            assert!(
                !atomic::staging_dir_for(&layout(), &spec).exists(),
                "staging should be consumed by the swap"
            );
            assert!(
                leftover.exists(),
                "the locked leftover cannot be deleted yet and simply waits"
            );
            assert!(
                cs::has_retired_leftover(&spec),
                "a later swap sweeps the leftover once its process has exited"
            );

            child.kill().expect("should kill holder child");
            child.wait().expect("should reap holder child");
        },
    );
}

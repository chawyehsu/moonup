use std::path::PathBuf;

use moonup::{
    constant,
    dist_server::schema::Release,
    toolchain::{ToolchainSpec, atomic, install::InstallRecipe, installed_toolchains},
};

fn spec(value: &str) -> ToolchainSpec {
    ToolchainSpec::try_from(value).expect("test spec should be valid")
}

fn staged_marker(spec: &ToolchainSpec, json: &str) {
    let marker = atomic::completeness_marker_for(spec);
    std::fs::create_dir_all(marker.parent().unwrap()).expect("should create .staging dir");
    std::fs::write(marker, json).expect("should write marker");
}

fn write_install_meta(path: &std::path::Path, spec: &ToolchainSpec) {
    std::fs::create_dir_all(path).expect("should create live dir");
    let meta = serde_json::json!({
        "channel": spec.channel(),
        "key": "0.1.0",
        "floating": matches!(spec, ToolchainSpec::Channel(_)),
    });
    std::fs::write(path.join("version"), meta.to_string()).expect("should write install metadata");
}

fn write_complete_marker(spec: &ToolchainSpec, version: &str) {
    let marker = serde_json::json!({
        "channel": spec.channel(),
        "floating": matches!(spec, ToolchainSpec::Channel(_)),
        "version": version,
    });
    staged_marker(spec, &marker.to_string());
}

fn swap_paths(spec: &ToolchainSpec) -> (PathBuf, PathBuf, PathBuf) {
    (
        spec.install_path(),
        atomic::staging_dir_for(spec),
        atomic::retired_dir_for(spec),
    )
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
            let live_dir = spec.install_path();
            let staging_dir = atomic::staging_dir_for(&spec);

            // simulate a crash left a fully assembled staging directory
            std::fs::create_dir_all(staging_dir.join("bin")).expect("should create staging dir");
            std::fs::write(staging_dir.join("bin").join("moon"), b"moon")
                .expect("should write staging content");
            staged_marker(&spec, r#"{"version":"0.1.0"}"#);

            assert!(!live_dir.exists());

            let staged = atomic::recover(&spec).expect("recovery should not fail");
            assert!(staged.is_some(), "complete staging should be promoted");
            assert_eq!(staged.unwrap().release.version, "0.1.0");

            assert!(
                live_dir.join("bin").join("moon").exists(),
                "staging should be promoted to the live location"
            );
            assert!(
                !staging_dir.exists(),
                "staging dir should be consumed by the swap"
            );
            assert!(
                atomic::completeness_marker_for(&spec).exists(),
                "the marker should be retained as a pending-finalization record"
            );

            atomic::acknowledge(&spec);
            assert!(
                !atomic::completeness_marker_for(&spec).exists(),
                "acknowledgment should remove the pending-finalization record"
            );
        },
    );
}

#[test]
fn test_recovery_recipe_uses_concrete_spec_for_stable_selector() {
    let staged = atomic::StagedRelease {
        channel: None,
        floating: None,
        release: Release::new("0.10.1+build"),
    };
    let requested = ToolchainSpec::Version {
        channel: "latest".to_string(),
        key: "0.10".to_string(),
    };

    let recipe = staged.into_recipe(&requested);

    assert_eq!(
        recipe.spec,
        ToolchainSpec::Version {
            channel: "latest".to_string(),
            key: "0.10.1+build".to_string()
        }
    );
    assert_eq!(recipe.requested_spec, requested);
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

            // a staging dir without the completeness marker was cut short
            let staging_dir = atomic::staging_dir_for(&spec);
            std::fs::create_dir_all(staging_dir.join("bin")).expect("should create staging dir");

            let staged = atomic::recover(&spec).expect("recovery should not fail");
            assert!(staged.is_none(), "partial staging should not be promoted");
            assert!(
                !spec.install_path().exists(),
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
            let live_dir = spec.install_path();

            // the live toolchain is already promoted but its finalization
            // never completed: the pending marker is still present and no
            // staging is being assembled
            write_install_meta(&live_dir, &spec);
            std::fs::create_dir_all(live_dir.join("bin")).expect("should create live bin dir");
            std::fs::write(live_dir.join("bin").join("moon"), b"moon")
                .expect("should write live content");
            staged_marker(&spec, r#"{"version":"0.1.0"}"#);

            let staged = atomic::recover(&spec).expect("recovery should not fail");
            assert!(staged.is_some(), "pending finalization should be retried");
            assert_eq!(staged.unwrap().release.version, "0.1.0");

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
            let live_dir = colliding_channel.install_path();
            write_install_meta(&live_dir, &colliding_channel);
            staged_marker(
                &pin,
                r#"{"channel":"lts","floating":false,"version":"2026-01-01"}"#,
            );

            let error = atomic::recover(&pin)
                .expect_err("recovery must not finalize another toolchain's live path");
            assert!(
                error.to_string().contains("occupied by"),
                "unexpected recovery error: {error}"
            );
            assert!(live_dir.exists(), "the existing install must be preserved");
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
            let (live_dir, staging_dir, retired_dir) = swap_paths(&spec);

            write_install_meta(&live_dir, &spec);
            std::fs::create_dir_all(live_dir.join("bin")).expect("should create live bin dir");
            std::fs::write(live_dir.join("bin").join("moon"), b"old")
                .expect("should write live content");
            std::fs::create_dir_all(staging_dir.join("bin")).expect("should create staging dir");
            std::fs::write(staging_dir.join("bin").join("moon"), b"new")
                .expect("should write staging content");
            write_complete_marker(&spec, "0.1.0");

            atomic::swap(&spec).expect("swap should succeed");

            assert_eq!(
                std::fs::read_to_string(live_dir.join("bin").join("moon"))
                    .expect("should read live moon"),
                "new"
            );
            assert!(!staging_dir.exists(), "staging should be consumed");
            assert!(!retired_dir.exists(), "retired dir should be removed");
            assert!(
                atomic::completeness_marker_for(&spec).exists(),
                "the marker should survive the swap as a pending-finalization record"
            );

            atomic::acknowledge(&spec);
            assert!(
                !atomic::completeness_marker_for(&spec).exists(),
                "acknowledgment should remove the pending-finalization record"
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
            let live_dir = colliding_channel.install_path();
            let staging_dir = atomic::staging_dir_for(&pin);

            write_install_meta(&live_dir, &colliding_channel);
            std::fs::create_dir_all(staging_dir.join("bin"))
                .expect("should create staging directory");
            write_complete_marker(&pin, "2026-01-01");

            assert!(atomic::swap(&pin).is_err());
            assert!(live_dir.exists(), "the existing install must be preserved");
            assert!(staging_dir.exists(), "the staged pin must be preserved");

            std::fs::remove_file(live_dir.join("version")).expect("should remove live metadata");
            assert!(atomic::swap(&pin).is_err());
            assert!(
                live_dir.exists(),
                "the unidentifiable path must be preserved"
            );
        },
    );
}

#[test]
fn test_swap_restores_retired_on_promotion_failure() {
    let tempdir = assert_fs::TempDir::new().expect("should create tempdir");
    let moonup_home = tempdir.path().join(".moonup");

    temp_env::with_var(
        constant::ENVNAME_MOONUP_HOME,
        Some(moonup_home.as_os_str()),
        || {
            let spec = spec("latest");
            let (live_dir, _, retired_dir) = swap_paths(&spec);

            // crash state: the previous live toolchain was moved to `.old`,
            // the promotion never happened, and the staging is gone
            std::fs::create_dir_all(retired_dir.join("bin")).expect("should create retired dir");
            std::fs::write(retired_dir.join("bin").join("moon"), b"old")
                .expect("should write retired content");

            let result = atomic::swap(&spec);
            assert!(
                result.is_err(),
                "promotion of a missing staging should fail"
            );

            assert!(
                live_dir.join("bin").join("moon").exists(),
                "retired toolchain should be restored to the live name"
            );
            assert_eq!(
                std::fs::read_to_string(live_dir.join("bin").join("moon"))
                    .expect("should read restored moon"),
                "old"
            );
            assert!(
                !retired_dir.exists(),
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
            std::fs::create_dir_all(atomic::staging_dir_for(&spec))
                .expect("should create staging dir");
            staged_marker(&spec, r#"{"version":"0.1.0"}"#);

            let recipe = |version: &str| InstallRecipe {
                spec: spec.clone(),
                requested_spec: spec.clone(),
                release: Release::new(version),
                components: Vec::new(),
            };

            assert!(
                atomic::staged_matches(&spec, &recipe("0.1.0"))
                    .expect("matching spec should be accepted"),
                "matching release should be promoted"
            );
            assert!(
                !atomic::staged_matches(&spec, &recipe("0.2.0"))
                    .expect("same spec with stale release should be accepted for cleanup"),
                "a stale staging must not be promoted over a newer release"
            );

            let pin =
                ToolchainSpec::try_from("lts@2026-01-01").expect("qualified pin should be valid");
            std::fs::create_dir_all(atomic::staging_dir_for(&pin))
                .expect("should create colliding staging dir");
            staged_marker(
                &pin,
                r#"{"channel":"lts-2026-01-01","floating":true,"version":"2026-01-01"}"#,
            );
            let colliding_recipe = InstallRecipe {
                spec: pin.clone(),
                requested_spec: pin.clone(),
                release: Release::new("2026-01-01"),
                components: Vec::new(),
            };
            assert!(
                atomic::staged_matches(&pin, &colliding_recipe).is_err(),
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
            let toolchains_dir = moonup_home.join("toolchains");
            std::fs::create_dir_all(toolchains_dir.join("latest"))
                .expect("should create toolchain dir");
            std::fs::write(toolchains_dir.join("latest").join("version"), "0.1.0\n")
                .expect("should write version stub");
            std::fs::create_dir_all(atomic::staging_dir_for(&spec("latest")))
                .expect("should create staging dir");

            let installed = installed_toolchains().expect("should list installed toolchains");
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

            let installed = installed_toolchains().expect("should list installed toolchains");
            let names = installed
                .iter()
                .map(|t| t.spec.to_string())
                .collect::<Vec<_>>();
            assert_eq!(names, vec!["0.10.0", "0.11.0"]);
        },
    );
}

#[test]
fn test_swap_sweeps_deletable_stale_retired() {
    let tempdir = assert_fs::TempDir::new().expect("should create tempdir");
    let moonup_home = tempdir.path().join(".moonup");

    temp_env::with_var(
        constant::ENVNAME_MOONUP_HOME,
        Some(moonup_home.as_os_str()),
        || {
            let spec = spec("latest");
            let (live_dir, staging_dir, retired_dir) = swap_paths(&spec);

            // a stale retired dir from a previous swap, now deletable
            std::fs::create_dir_all(retired_dir.join("bin")).expect("should create retired dir");
            std::fs::write(retired_dir.join("bin").join("moon"), b"old")
                .expect("should write retired content");

            write_install_meta(&live_dir, &spec);
            std::fs::create_dir_all(live_dir.join("bin")).expect("should create live bin dir");
            std::fs::write(live_dir.join("bin").join("moon"), b"live")
                .expect("should write live content");
            std::fs::create_dir_all(staging_dir.join("bin")).expect("should create staging dir");
            std::fs::write(staging_dir.join("bin").join("moon"), b"new")
                .expect("should write staging content");
            write_complete_marker(&spec, "0.1.0");

            atomic::swap(&spec).expect("swap should succeed");

            assert_eq!(
                std::fs::read_to_string(live_dir.join("bin").join("moon"))
                    .expect("should read live moon"),
                "new"
            );
            assert!(
                !staging_dir.exists(),
                "staging should be consumed by the swap"
            );
            assert!(
                !retired_dir.exists(),
                "deletable stale retired dir should be swept, not shunted"
            );
            assert!(
                !has_retired_leftover(&spec),
                "no retired dir or tombstone should remain after sweeping a deletable stale retired"
            );
        },
    );
}

#[test]
fn test_swap_sweeps_stale_tombstone() {
    let tempdir = assert_fs::TempDir::new().expect("should create tempdir");
    let moonup_home = tempdir.path().join(".moonup");

    temp_env::with_var(
        constant::ENVNAME_MOONUP_HOME,
        Some(moonup_home.as_os_str()),
        || {
            let spec = spec("latest");
            let (live_dir, staging_dir, retired_dir) = swap_paths(&spec);

            // a stale retired dir and an even older shunted tombstone from a
            // previous swap, all now deletable
            std::fs::create_dir_all(retired_dir.join("bin")).expect("should create retired dir");
            std::fs::write(retired_dir.join("bin").join("moon"), b"old")
                .expect("should write retired content");
            let tombstone = retired_dir
                .parent()
                .expect("should have .staging parent")
                .join(format!("latest.old.{}.0", std::process::id()));
            std::fs::create_dir_all(tombstone.join("bin")).expect("should create tombstone dir");

            write_install_meta(&live_dir, &spec);
            std::fs::create_dir_all(live_dir.join("bin")).expect("should create live bin dir");
            std::fs::write(live_dir.join("bin").join("moon"), b"live")
                .expect("should write live content");
            std::fs::create_dir_all(staging_dir.join("bin")).expect("should create staging dir");
            std::fs::write(staging_dir.join("bin").join("moon"), b"new")
                .expect("should write staging content");
            write_complete_marker(&spec, "0.1.0");

            atomic::swap(&spec).expect("swap should succeed");

            assert_eq!(
                std::fs::read_to_string(live_dir.join("bin").join("moon"))
                    .expect("should read live moon"),
                "new"
            );
            assert!(
                !staging_dir.exists(),
                "staging should be consumed by the swap"
            );
            assert!(
                !has_retired_leftover(&spec),
                "stale retired dir and tombstone should both be swept"
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
            let staging_dir = atomic::staging_dir_for(&spec);
            std::fs::create_dir_all(&staging_dir).expect("should create staging dir");

            let latest_old = atomic::retired_dir_for(&spec);
            std::fs::create_dir_all(&latest_old).expect("should create latest.old");
            std::fs::create_dir_all(
                latest_old
                    .parent()
                    .expect("should have .staging parent")
                    .join(format!("latest.old.{}.0", std::process::id())),
            )
            .expect("should create latest tombstone");

            // a versioned nightly spec the `latest` sweep must not touch
            let nightly_old = latest_old
                .parent()
                .expect("should have .staging parent")
                .join("nightly-2025-01-01.old");
            std::fs::create_dir_all(&nightly_old).expect("should create nightly-versioned old");

            atomic::sweep_staging(&spec);

            assert!(!latest_old.exists(), "latest.old should be swept");
            assert!(
                !staging_dir.exists(),
                "latest.new staging dir should be swept"
            );
            assert!(
                !has_retired_leftover(&spec),
                "latest tombstones should be swept"
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
fn test_swap_shunts_locked_retired() {
    use std::process::Command;
    use std::time::Duration;

    // Child mode: keep a real running image mapped inside the stale retired
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
            let (live_dir, staging_dir, retired_dir) = swap_paths(&spec);

            let exe = std::env::current_exe().expect("should resolve current exe");

            // the stale retired dir holds a real running image: it can be
            // renamed but not deleted
            std::fs::create_dir_all(retired_dir.join("bin")).expect("should create retired dir");
            std::fs::copy(&exe, retired_dir.join("bin").join("holder.exe"))
                .expect("should copy current exe");
            let mut child = Command::new(retired_dir.join("bin").join("holder.exe"))
                .arg("test_swap_shunts_locked_retired")
                .env("MOONUP_TEST_HOLD", "1")
                .spawn()
                .expect("should spawn holder child");
            std::thread::sleep(Duration::from_millis(500));

            write_install_meta(&live_dir, &spec);
            std::fs::create_dir_all(live_dir.join("bin")).expect("should create live bin dir");
            std::fs::write(live_dir.join("bin").join("moon"), b"live")
                .expect("should write live content");
            std::fs::create_dir_all(staging_dir.join("bin")).expect("should create staging dir");
            std::fs::write(staging_dir.join("bin").join("moon"), b"new")
                .expect("should write staging content");
            write_complete_marker(&spec, "0.1.0");

            atomic::swap(&spec).expect("swap should tolerate a locked retired dir");

            assert_eq!(
                std::fs::read_to_string(live_dir.join("bin").join("moon"))
                    .expect("should read live moon"),
                "new"
            );
            assert!(
                !retired_dir.exists(),
                "locked retired dir should be shunted aside, freeing the .old slot"
            );
            assert!(
                has_retired_leftover(&spec),
                "the shunted tombstone should still hold the locked image"
            );

            child.kill().expect("should kill holder child");
            child.wait().expect("should reap holder child");
        },
    );
}

fn has_retired_leftover(spec: &ToolchainSpec) -> bool {
    let staging_dir = atomic::staging_dir_for(spec);
    let prefix = format!("{}.old", spec.install_name());
    let Ok(read_dir) = staging_dir
        .parent()
        .expect("staging dir should have a parent")
        .read_dir()
    else {
        return false;
    };
    read_dir.flatten().any(|e| {
        let file_name = e.file_name();
        let name = file_name.to_string_lossy();
        name.starts_with(&prefix)
    })
}

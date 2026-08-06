use std::path::PathBuf;

use moonup::{
    constant,
    dist_server::schema::Release,
    toolchain::{ToolchainSpec, atomic, index::InstallRecipe, installed_toolchains},
};

fn staged_marker(spec: &ToolchainSpec, json: &str) {
    let marker = atomic::completeness_marker_for(spec);
    std::fs::create_dir_all(marker.parent().unwrap()).expect("should create .staging dir");
    std::fs::write(marker, json).expect("should write marker");
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
            let spec = ToolchainSpec::Latest;
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
            assert_eq!(staged.unwrap().version, "0.1.0");

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
fn test_recover_does_not_promote_incomplete_staging() {
    let tempdir = assert_fs::TempDir::new().expect("should create tempdir");
    let moonup_home = tempdir.path().join(".moonup");

    temp_env::with_var(
        constant::ENVNAME_MOONUP_HOME,
        Some(moonup_home.as_os_str()),
        || {
            let spec = ToolchainSpec::Latest;

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
            let spec = ToolchainSpec::Latest;
            let live_dir = spec.install_path();

            // the live toolchain is already promoted but its finalization
            // never completed: the pending marker is still present and no
            // staging is being assembled
            std::fs::create_dir_all(live_dir.join("bin")).expect("should create live dir");
            std::fs::write(live_dir.join("bin").join("moon"), b"moon")
                .expect("should write live content");
            staged_marker(&spec, r#"{"version":"0.1.0"}"#);

            let staged = atomic::recover(&spec).expect("recovery should not fail");
            assert!(staged.is_some(), "pending finalization should be retried");
            assert_eq!(staged.unwrap().version, "0.1.0");

            assert!(
                live_dir.join("bin").join("moon").exists(),
                "live toolchain should be left untouched"
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
            let spec = ToolchainSpec::Latest;
            let (live_dir, staging_dir, retired_dir) = swap_paths(&spec);

            std::fs::create_dir_all(live_dir.join("bin")).expect("should create live dir");
            std::fs::write(live_dir.join("bin").join("moon"), b"old")
                .expect("should write live content");
            std::fs::create_dir_all(staging_dir.join("bin")).expect("should create staging dir");
            std::fs::write(staging_dir.join("bin").join("moon"), b"new")
                .expect("should write staging content");
            staged_marker(&spec, r#"{"version":"0.1.0"}"#);

            atomic::swap(&live_dir, &staging_dir).expect("swap should succeed");

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
fn test_swap_restores_retired_on_promotion_failure() {
    let tempdir = assert_fs::TempDir::new().expect("should create tempdir");
    let moonup_home = tempdir.path().join(".moonup");

    temp_env::with_var(
        constant::ENVNAME_MOONUP_HOME,
        Some(moonup_home.as_os_str()),
        || {
            let spec = ToolchainSpec::Latest;
            let (live_dir, staging_dir, retired_dir) = swap_paths(&spec);

            // crash state: the previous live toolchain was moved to `.old`,
            // the promotion never happened, and the staging is gone
            std::fs::create_dir_all(retired_dir.join("bin")).expect("should create retired dir");
            std::fs::write(retired_dir.join("bin").join("moon"), b"old")
                .expect("should write retired content");

            let result = atomic::swap(&live_dir, &staging_dir);
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
            let spec = ToolchainSpec::Latest;
            std::fs::create_dir_all(atomic::staging_dir_for(&spec))
                .expect("should create staging dir");
            staged_marker(&spec, r#"{"version":"0.1.0"}"#);

            let recipe = |version: &str| InstallRecipe {
                spec: spec.clone(),
                release: Release {
                    version: version.to_string(),
                    layout_version1: None,
                    bundle_source_dir: None,
                    date: None,
                    targets: None,
                },
                components: Vec::new(),
            };

            assert!(
                atomic::staged_matches(&spec, &recipe("0.1.0")),
                "matching release should be promoted"
            );
            assert!(
                !atomic::staged_matches(&spec, &recipe("0.2.0")),
                "a stale staging must not be promoted over a newer release"
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
            std::fs::create_dir_all(atomic::staging_dir_for(&ToolchainSpec::Latest))
                .expect("should create staging dir");

            let installed = installed_toolchains().expect("should list installed toolchains");
            assert_eq!(installed.len(), 1);
            assert_eq!(installed[0].name, ToolchainSpec::Latest);
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
            let spec = ToolchainSpec::Latest;
            let (live_dir, staging_dir, retired_dir) = swap_paths(&spec);

            // a stale retired dir from a previous swap, now deletable
            std::fs::create_dir_all(retired_dir.join("bin")).expect("should create retired dir");
            std::fs::write(retired_dir.join("bin").join("moon"), b"old")
                .expect("should write retired content");

            std::fs::create_dir_all(live_dir.join("bin")).expect("should create live dir");
            std::fs::write(live_dir.join("bin").join("moon"), b"live")
                .expect("should write live content");
            std::fs::create_dir_all(staging_dir.join("bin")).expect("should create staging dir");
            std::fs::write(staging_dir.join("bin").join("moon"), b"new")
                .expect("should write staging content");

            atomic::swap(&live_dir, &staging_dir).expect("swap should succeed");

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
            let spec = ToolchainSpec::Latest;
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

            std::fs::create_dir_all(live_dir.join("bin")).expect("should create live dir");
            std::fs::write(live_dir.join("bin").join("moon"), b"live")
                .expect("should write live content");
            std::fs::create_dir_all(staging_dir.join("bin")).expect("should create staging dir");
            std::fs::write(staging_dir.join("bin").join("moon"), b"new")
                .expect("should write staging content");

            atomic::swap(&live_dir, &staging_dir).expect("swap should succeed");

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
            let spec = ToolchainSpec::Latest;
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
            let spec = ToolchainSpec::Latest;
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

            std::fs::create_dir_all(live_dir.join("bin")).expect("should create live dir");
            std::fs::write(live_dir.join("bin").join("moon"), b"live")
                .expect("should write live content");
            std::fs::create_dir_all(staging_dir.join("bin")).expect("should create staging dir");
            std::fs::write(staging_dir.join("bin").join("moon"), b"new")
                .expect("should write staging content");

            atomic::swap(&live_dir, &staging_dir)
                .expect("swap should tolerate a locked retired dir");

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
    let prefix = format!("{}.old", spec.as_str());
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

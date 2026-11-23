//! End-to-end tests for atomic toolchain recovery.
//!
//! These tests run in default CI with zero network: `MOONUP_DIST_SERVER` points
//! at a local mockito server serving the committed dist-server fixtures,
//! including tiny per-target toolchain stub archives.

use std::path::PathBuf;

use moonup::{
    constant,
    toolchain::{ToolchainSpec, atomic},
};

use crate::{
    constant::E2E_TEST_MOCK_INSTALL_VERSION,
    util::{TestWorkspace, mock_dist_server},
};

fn spec(value: &str) -> ToolchainSpec {
    ToolchainSpec::try_from(value).expect("test spec should be valid")
}

/// The staging leftovers a recovery is expected to consume.
fn staging_leftovers(spec: &ToolchainSpec) -> (PathBuf, PathBuf) {
    (
        atomic::staging_dir_for(spec),
        atomic::completeness_marker_for(spec),
    )
}

/// Simulate a crash that left a fully assembled staging directory: move the
/// live toolchain into the staging location and write the completeness marker.
fn simulate_crash_state(ws: &TestWorkspace, name: &str, version: &str) {
    let spec = spec(name);

    // the path helpers resolve `MOONUP_HOME` from the process environment, so
    // point it at the test workspace while computing the crash state
    temp_env::with_var(
        constant::ENVNAME_MOONUP_HOME,
        Some(ws.moonup_home().as_os_str()),
        || {
            let install_path = spec.install_path();
            let (staging_dir, marker) = staging_leftovers(&spec);

            std::fs::create_dir_all(staging_dir.parent().unwrap())
                .expect("should create .staging dir");
            std::fs::rename(&install_path, &staging_dir)
                .expect("should move toolchain into staging");
            std::fs::write(&marker, format!(r#"{{"version":"{version}"}}"#))
                .expect("should write marker");

            assert!(!install_path.exists());
        },
    );
}

#[test]
fn test_install_recovers_interrupted_swap() {
    let ws = TestWorkspace::new();
    let mut s = mock_dist_server();
    let spec = spec(E2E_TEST_MOCK_INSTALL_VERSION);

    temp_env::with_var(
        constant::ENVNAME_MOONUP_HOME,
        Some(ws.moonup_home().as_os_str()),
        || {
            let install_path = spec.install_path();
            let (staging_dir, marker) = staging_leftovers(&spec);

            // first, install the toolchain
            let _ = ws
                .cli()
                .env(constant::ENVNAME_MOONUP_DIST_SERVER, s.url())
                .arg("install")
                .arg(E2E_TEST_MOCK_INSTALL_VERSION)
                .output()
                .expect("should run moonup install");
            assert!(install_path.exists());

            // remove the poured shim so a successful recovery is provable by
            // its post-install steps re-pouring it
            let shim = ws.bin("moon");
            assert!(shim.exists(), "install should have poured the moon shim");
            std::fs::remove_file(&shim).expect("should remove the moon shim");
            assert!(!shim.exists());

            // simulate a crash that left a complete staging directory
            simulate_crash_state(
                &ws,
                E2E_TEST_MOCK_INSTALL_VERSION,
                E2E_TEST_MOCK_INSTALL_VERSION,
            );

            // With the index cache removed, recovery should still complete
            // locally without requesting the main index.
            let downloads = ws.moonup_home().join("downloads");
            std::fs::remove_file(downloads.join("index.json"))
                .expect("should remove cached main index");
            std::fs::remove_file(downloads.join("index.json.lock"))
                .expect("should remove cached main index lock");
            s.reset();
            let index_request = s.mock("GET", "/index.json").expect(0).create();

            // the next install should recover the staging instead of
            // re-downloading
            let _ = ws
                .cli()
                .env(constant::ENVNAME_MOONUP_DIST_SERVER, s.url())
                .arg("install")
                .arg(E2E_TEST_MOCK_INSTALL_VERSION)
                .output()
                .expect("should run moonup install");
            index_request.assert();

            // the promoted toolchain is live again and the staging is consumed
            assert!(
                install_path.exists(),
                "toolchain should be promoted back to the live location"
            );
            assert!(!staging_dir.exists(), "staging dir should be consumed");
            assert!(!marker.exists(), "completeness marker should be cleaned up");

            // the recovery finalized the installation: the shim was re-poured
            assert!(shim.exists(), "shim should have re-poured after recovery");
        },
    );
}

#[test]
fn test_install_repairs_shims_for_up_to_date_toolchain() {
    let ws = TestWorkspace::new();
    let s = mock_dist_server();

    let output = ws
        .cli()
        .env(constant::ENVNAME_MOONUP_DIST_SERVER, s.url())
        .arg("install")
        .arg("latest")
        .output()
        .expect("should install latest toolchain");
    assert!(output.status.success(), "initial install should succeed");

    let alternate_moon_home = ws.tempdir().path().join("alternate-moon-home");
    let alternate_moon_bin = alternate_moon_home.join("bin");
    let moon_shim = alternate_moon_bin.join(if cfg!(windows) { "moon.exe" } else { "moon" });
    assert!(
        !moon_shim.exists(),
        "alternate MOON_HOME starts without shims"
    );

    let mut command = ws.cli();
    command
        .env(constant::ENVNAME_MOONUP_DIST_SERVER, s.url())
        .env(constant::ENVNAME_MOON_HOME, &alternate_moon_home);
    let output = command
        .arg("install")
        .arg("latest")
        .output()
        .expect("should reinstall latest toolchain");
    assert!(output.status.success(), "reinstall should succeed");
    assert!(
        moon_shim.exists(),
        "explicit install should recreate command shims in the current MOON_HOME"
    );
}

#[test]
fn test_update_skips_incomplete_staging_without_index_lookup() {
    let ws = TestWorkspace::new();
    let mut s = mock_dist_server();
    let staging = ws
        .moonup_home()
        .join("toolchains")
        .join(".staging")
        .join("latest.new");
    std::fs::create_dir_all(&staging).expect("should create incomplete staging directory");

    s.reset();
    let index_request = s.mock("GET", "/index.json").expect(0).create();
    let output = ws
        .cli()
        .env(constant::ENVNAME_MOONUP_DIST_SERVER, s.url())
        .arg("update")
        .output()
        .expect("should run update");

    assert!(
        output.status.success(),
        "uninstalled channel should be skipped"
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("No channel toolchains installed"),
        "update should only report installed channel toolchains"
    );
    index_request.assert();
}

#[test]
fn test_update_ignores_staged_toolchain_and_install_recovers_it() {
    let ws = TestWorkspace::new();
    let s = mock_dist_server();
    let spec = spec("latest");

    temp_env::with_var(
        constant::ENVNAME_MOONUP_HOME,
        Some(ws.moonup_home().as_os_str()),
        || {
            let install_path = spec.install_path();
            let (staging_dir, marker) = staging_leftovers(&spec);

            // install the `latest` channel first
            let _ = ws
                .cli()
                .env(constant::ENVNAME_MOONUP_DIST_SERVER, s.url())
                .arg("install")
                .arg("latest")
                .output()
                .expect("should run moonup install");

            let actual_version = std::fs::read_to_string(install_path.join("version"))
                .expect("should read metadata stub");
            let actual_version =
                serde_json::from_str::<moonup::toolchain::InstallMeta>(actual_version.trim())
                    .expect("should parse metadata stub")
                    .key;

            // simulate a crash that left a complete staging directory
            simulate_crash_state(&ws, "latest", &actual_version);

            let _ = ws
                .cli()
                .env(constant::ENVNAME_MOONUP_DIST_SERVER, s.url())
                .arg("update")
                .output()
                .expect("should run moonup update");
            assert!(
                !install_path.exists(),
                "update should not recover latest toolchain"
            );

            // install explicitly resumes recovery
            let _ = ws
                .cli()
                .env(constant::ENVNAME_MOONUP_DIST_SERVER, s.url())
                .arg("install")
                .arg("latest")
                .output()
                .expect("should run moonup install");

            // the promoted toolchain is live again and the staging is consumed
            assert!(
                install_path.exists(),
                "latest toolchain should be promoted back to the live location"
            );
            assert!(!staging_dir.exists(), "staging dir should be consumed");
            assert!(!marker.exists(), "completeness marker should be cleaned up");
        },
    );
}

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

use crate::util::{TestWorkspace, mock_dist_server};

/// A toolchain version available from the committed dist-server fixtures
/// (shrunk to a stub `moon` that only execs and exits 0).
const TEST_INSTALL_VERSION: &str = "0.10.1+a46be2066";

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
    let spec = ToolchainSpec::from(name);

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
    let (_server, dist_server_url) = mock_dist_server();
    let spec = ToolchainSpec::from(TEST_INSTALL_VERSION);

    temp_env::with_var(
        constant::ENVNAME_MOONUP_HOME,
        Some(ws.moonup_home().as_os_str()),
        || {
            let install_path = spec.install_path();
            let (staging_dir, marker) = staging_leftovers(&spec);

            // first, install the toolchain
            let _ = ws
                .cli()
                .env(
                    constant::ENVNAME_MOONUP_DIST_SERVER,
                    dist_server_url.as_str(),
                )
                .arg("install")
                .arg(TEST_INSTALL_VERSION)
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
            simulate_crash_state(&ws, TEST_INSTALL_VERSION, TEST_INSTALL_VERSION);

            // the next install should recover the staging instead of
            // re-downloading
            let _ = ws
                .cli()
                .env(
                    constant::ENVNAME_MOONUP_DIST_SERVER,
                    dist_server_url.as_str(),
                )
                .arg("install")
                .arg(TEST_INSTALL_VERSION)
                .output()
                .expect("should run moonup install");

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
fn test_update_recovers_interrupted_swap() {
    let ws = TestWorkspace::new();
    let (_server, dist_server_url) = mock_dist_server();
    let spec = ToolchainSpec::Latest;

    temp_env::with_var(
        constant::ENVNAME_MOONUP_HOME,
        Some(ws.moonup_home().as_os_str()),
        || {
            let install_path = spec.install_path();
            let (staging_dir, marker) = staging_leftovers(&spec);

            // install the `latest` channel first
            let _ = ws
                .cli()
                .env(
                    constant::ENVNAME_MOONUP_DIST_SERVER,
                    dist_server_url.as_str(),
                )
                .arg("install")
                .arg("latest")
                .output()
                .expect("should run moonup install");

            let actual_version = std::fs::read_to_string(install_path.join("version"))
                .expect("should read version stub")
                .trim()
                .to_string();

            // simulate a crash that left a complete staging directory
            simulate_crash_state(&ws, "latest", &actual_version);

            // the next update should recover the latest toolchain
            let _ = ws
                .cli()
                .env(
                    constant::ENVNAME_MOONUP_DIST_SERVER,
                    dist_server_url.as_str(),
                )
                .arg("update")
                .output()
                .expect("should run moonup update");

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

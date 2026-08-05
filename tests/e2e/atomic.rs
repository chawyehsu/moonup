//! End-to-end tests for atomic toolchain recovery.
//!
//! These tests require a real installation (real downloads), so they are
//! guarded behind the `test-liveinstall` feature flag.

use serial_test::serial;
use std::path::PathBuf;

use crate::util::TestWorkspace;

/// A toolchain version that is available from the test distribution server.
const TEST_INSTALL_VERSION: &str = "0.1.20241231+ba15a9a4e";

/// Simulate a crash that left a fully assembled staging directory: move the
/// live toolchain into the staging location and write the completeness marker.
fn simulate_crash_state(ws: &TestWorkspace, name: &str, version: &str) {
    let install_path = ws.moonup_home().join("toolchains").join(name);
    let staging_dir = ws
        .moonup_home()
        .join("toolchains")
        .join(".staging")
        .join(format!("{name}.new"));
    let marker = ws
        .moonup_home()
        .join("toolchains")
        .join(".staging")
        .join(format!("{name}.complete"));

    std::fs::create_dir_all(staging_dir.parent().unwrap()).expect("should create .staging dir");
    std::fs::rename(&install_path, &staging_dir).expect("should move toolchain into staging");
    std::fs::write(&marker, format!(r#"{{"version":"{version}"}}"#)).expect("should write marker");

    assert!(!install_path.exists());
}

fn staging_leftovers(ws: &TestWorkspace, name: &str) -> (PathBuf, PathBuf) {
    let staging_dir = ws
        .moonup_home()
        .join("toolchains")
        .join(".staging")
        .join(format!("{name}.new"));
    let marker = ws
        .moonup_home()
        .join("toolchains")
        .join(".staging")
        .join(format!("{name}.complete"));
    (staging_dir, marker)
}

#[cfg(windows)]
fn shim_exe_name() -> &'static str {
    "moon.exe"
}

#[cfg(not(windows))]
fn shim_exe_name() -> &'static str {
    "moon"
}

#[test]
#[serial]
fn test_install_recovers_interrupted_swap() {
    let ws = TestWorkspace::new();

    // first, install the toolchain
    assert!(
        ws.cli()
            .arg("install")
            .arg(TEST_INSTALL_VERSION)
            .status()
            .expect("should run moonup install")
            .success()
    );

    let install_path = ws
        .moonup_home()
        .join("toolchains")
        .join(TEST_INSTALL_VERSION);
    assert!(install_path.exists());

    // remove the poured shim so a successful recovery is provable by its
    // post-install steps re-pouring it
    let moon_shim = ws.moon_home().join("bin").join(shim_exe_name());
    assert!(
        moon_shim.exists(),
        "install should have poured the moon shim"
    );
    std::fs::remove_file(&moon_shim).expect("should remove the moon shim");
    assert!(!moon_shim.exists());

    // simulate a crash that left a complete staging directory
    simulate_crash_state(&ws, TEST_INSTALL_VERSION, TEST_INSTALL_VERSION);
    let (staging_dir, marker) = staging_leftovers(&ws, TEST_INSTALL_VERSION);

    // the next install should recover the staging instead of re-downloading
    assert!(
        ws.cli()
            .arg("install")
            .arg(TEST_INSTALL_VERSION)
            .status()
            .expect("should run moonup install")
            .success()
    );

    // the promoted toolchain is live again and the staging is consumed
    assert!(
        install_path.exists(),
        "toolchain should be promoted back to the live location"
    );
    assert!(!staging_dir.exists(), "staging dir should be consumed");
    assert!(!marker.exists(), "completeness marker should be cleaned up");

    // the recovery finalized the installation: the shim was re-poured
    assert!(
        moon_shim.exists(),
        "post-install should have re-poured the moon shim after recovery"
    );
}

#[test]
#[serial]
fn test_update_recovers_interrupted_swap() {
    let ws = TestWorkspace::new();

    // install the `latest` channel first
    assert!(
        ws.cli()
            .arg("install")
            .arg("latest")
            .status()
            .expect("should run moonup install")
            .success()
    );

    let install_path = ws.moonup_home().join("toolchains").join("latest");
    let actual_version = std::fs::read_to_string(install_path.join("version"))
        .expect("should read version stub")
        .trim()
        .to_string();

    // simulate a crash that left a complete staging directory
    simulate_crash_state(&ws, "latest", &actual_version);
    let (staging_dir, marker) = staging_leftovers(&ws, "latest");

    // the next update should recover the latest toolchain
    assert!(
        ws.cli()
            .arg("update")
            .status()
            .expect("should run moonup update")
            .success()
    );

    // the promoted toolchain is live again and the staging is consumed
    assert!(
        install_path.exists(),
        "latest toolchain should be promoted back to the live location"
    );
    assert!(!staging_dir.exists(), "staging dir should be consumed");
    assert!(!marker.exists(), "completeness marker should be cleaned up");
}

//! End-to-end tests for atomic toolchain recovery.
//!
//! These tests run in default CI with zero network: `MOONUP_DIST_SERVER` points
//! at a local mockito server serving the committed dist-server fixtures,
//! including tiny per-target toolchain stub archives.

use moonup::{
    constant,
    toolchain::{ToolchainSpec, atomic},
};

use crate::{
    constant::E2E_TEST_MOCK_INSTALL_VERSION,
    util::{TestWorkspace, crash_state as cs, mock_dist_server},
};

fn layout() -> moonup::layout::Layout {
    moonup::layout::Layout::from_env()
}

fn spec(value: &str) -> ToolchainSpec {
    ToolchainSpec::try_from(value).expect("test spec should be valid")
}

/// Simulate a crash that left a fully assembled staging directory: move the
/// live toolchain into the staging location and rewrite its install record
/// with a pending finalization payload, as `populate` would have written
/// before the swap it did not survive.
fn simulate_crash_state(ws: &TestWorkspace, name: &str, key: &str) {
    let spec = spec(name);

    // the path helpers resolve `MOONUP_HOME` from the process environment, so
    // point it at the test workspace while computing the crash state
    temp_env::with_var(
        constant::ENVNAME_MOONUP_HOME,
        Some(ws.moonup_home().as_os_str()),
        || {
            let install_path = layout().toolchain_dir(&spec);
            let staging_dir = atomic::staging_dir_for(&layout(), &spec);

            std::fs::create_dir_all(staging_dir.parent().unwrap())
                .expect("should create .staging dir");
            std::fs::rename(&install_path, &staging_dir)
                .expect("should move toolchain into staging");
            let finalization = serde_json::json!({"bundle_source_dir": false});
            std::fs::write(
                staging_dir.join("version"),
                cs::install_record(&spec, key, Some(finalization)),
            )
            .expect("should rewrite the staged record");

            assert!(!install_path.exists());
        },
    );
}

#[test]
fn test_install_recovers_interrupted_swap() {
    let ws = TestWorkspace::new();
    let s = mock_dist_server();
    let spec = spec(E2E_TEST_MOCK_INSTALL_VERSION);

    temp_env::with_var(
        constant::ENVNAME_MOONUP_HOME,
        Some(ws.moonup_home().as_os_str()),
        || {
            let install_path = layout().toolchain_dir(&spec);

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

            // the next install should recover the staging instead of
            // re-downloading
            let _ = ws
                .cli()
                .env(constant::ENVNAME_MOONUP_DIST_SERVER, s.url())
                .arg("install")
                .arg(E2E_TEST_MOCK_INSTALL_VERSION)
                .output()
                .expect("should run moonup install");

            // the promoted toolchain is live again and the staging is consumed
            assert!(
                install_path.exists(),
                "toolchain should be promoted back to the live location"
            );
            assert!(
                !atomic::staging_dir_for(&layout(), &spec).exists(),
                "staging dir should be consumed"
            );
            assert!(
                !cs::live_record_has_finalization(&spec),
                "recovery should acknowledge the promotion in the install record"
            );

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

    temp_env::with_var(
        constant::ENVNAME_MOONUP_HOME,
        Some(ws.moonup_home().as_os_str()),
        || {
            cs::write_partial_staging(&spec("latest"));
        },
    );

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
            let install_path = layout().toolchain_dir(&spec);

            // install the `latest` channel first
            let _ = ws
                .cli()
                .env(constant::ENVNAME_MOONUP_DIST_SERVER, s.url())
                .arg("install")
                .arg("latest")
                .output()
                .expect("should run moonup install");

            let actual_key = std::fs::read_to_string(install_path.join("version"))
                .expect("should read install record");
            let actual_key =
                serde_json::from_str::<moonup::toolchain::InstallMeta>(actual_key.trim())
                    .expect("should parse install record")
                    .key;

            // simulate a crash that left a complete staging directory
            simulate_crash_state(&ws, "latest", &actual_key);

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
            assert!(
                !atomic::staging_dir_for(&layout(), &spec).exists(),
                "staging dir should be consumed"
            );
            assert!(
                !cs::live_record_has_finalization(&spec),
                "recovery should acknowledge the promotion in the install record"
            );
        },
    );
}

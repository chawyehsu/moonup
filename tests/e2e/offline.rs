//! End-to-end tests that exercise the mock dist server (committed fixtures).
//!
//! `online.rs` holds tests that involve the production dist server; the tests
//! here run against `mock_dist_server()` with zero network.

use std::{env, fs};

use insta_cmd::assert_cmd_snapshot;
use moonup::constant;
use serial_test::serial;

use crate::util::{self, TestWorkspace};

#[test]
#[serial]
fn moonup_pin() {
    util::apply_common_filters!();
    let ws = TestWorkspace::new();
    let project_path = ws.project_path();

    fs::create_dir_all(project_path).expect("should create project directory");
    env::set_current_dir(project_path).expect("should set current directory");

    // Pin toolchain
    assert_cmd_snapshot!("moonup_pin", ws.cli().arg("pin").arg("nightly"));
    assert!(project_path.join(constant::TOOLCHAIN_FILE).exists());

    // Pin, but no toolchain installed, should show subcommand help
    assert_cmd_snapshot!("moonup_pin_help_fallback", ws.cli().arg("pin"));

    env::set_current_dir(ws.tempdir()).expect("should restore current directory");
}

#[test]
fn moonup_default() {
    util::apply_common_filters!();
    let ws = TestWorkspace::new();

    // Set default toolchain
    assert_cmd_snapshot!("moonup_default", ws.cli().arg("default").arg("latest"));

    // Set default toolchain interactively, but no toolchain installed,
    // should show subcommand help
    assert_cmd_snapshot!("moonup_default_help_fallback", ws.cli().arg("default"));
}

#[test]
fn moonup_others() {
    util::apply_common_filters!();
    let ws = TestWorkspace::new();

    assert_cmd_snapshot!("moonup_completions", ws.cli().arg("completions").arg("zsh"));

    // No toolchain installed
    assert_cmd_snapshot!("moonup_show", ws.cli().arg("show"));
}

#[test]
fn moonup_run() {
    util::apply_common_filters!();
    let ws = TestWorkspace::new();

    assert_cmd_snapshot!(
        "moonup_run_not_installed",
        ws.cli().arg("run").arg("nightly").arg("moon").arg("--help")
    );
}

#[test]
fn moonup_install() {
    util::apply_common_filters!();
    let ws = TestWorkspace::new();

    // setup mock server for dist_server from committed fixtures
    let s = util::mock_dist_server();

    assert_cmd_snapshot!(
        "moonup_install",
        ws.cli()
            .env(constant::ENVNAME_MOONUP_DIST_SERVER, s.url())
            .arg("install")
    );

    // Override the dist server URL with the mock server URL
    assert_cmd_snapshot!(
        "moonup_install_list_available",
        ws.cli()
            .env(constant::ENVNAME_MOONUP_DIST_SERVER, s.url())
            .arg("install")
            .arg("--list-available")
    );

    // Should hit the cache and return the same result
    assert_cmd_snapshot!(
        "moonup_install_list_available_verbose",
        ws.cli()
            .env(constant::ENVNAME_MOONUP_DIST_SERVER, s.url())
            .arg("install")
            .arg("--list-available")
            .arg("-vvv")
    );

    // No available toolchains for the channel
    assert_cmd_snapshot!(
        "moonup_install_no_available_toolchains",
        ws.cli()
            .env(constant::ENVNAME_MOONUP_DIST_SERVER, s.url())
            .arg("install")
            .arg("nightly")
            .arg("--list-available")
    );
}

#[test]
fn offline_flow() {
    util::apply_common_filters!();
    let ws = TestWorkspace::new();

    // setup mock server for dist_server from committed fixtures
    let s = util::mock_dist_server();

    // Install the latest toolchain
    let output = ws
        .cli()
        .env(constant::ENVNAME_MOONUP_DIST_SERVER, s.url())
        .arg("install")
        .arg("latest")
        .output()
        .expect("should install latest toolchain successfully");
    assert!(output.status.success());

    assert_cmd_snapshot!(
        "moonup_run_command_unavailable",
        ws.cli()
            .env(constant::ENVNAME_MOONUP_DIST_SERVER, s.url())
            .arg("run")
            .arg("latest")
            .arg("bad")
    );

    let exitstatus = ws
        .cli()
        .env(constant::ENVNAME_MOONUP_DIST_SERVER, s.url())
        .arg("run")
        .arg("latest")
        .arg("moon")
        .arg("subcommand")
        .status()
        .expect("should run moon subcommand and get nonzero exit code");
    assert!(!exitstatus.success());

    assert_cmd_snapshot!(
        "moonup_which_command_unavailable",
        ws.cli()
            .env(constant::ENVNAME_MOONUP_DIST_SERVER, s.url())
            .arg("which")
            .arg("bad")
    );

    // Uninstall the toolchain
    let output = ws
        .cli()
        .env(constant::ENVNAME_MOONUP_DIST_SERVER, s.url())
        .arg("uninstall")
        .arg("latest")
        .output()
        .expect("should uninstall latest toolchain successfully");
    assert!(output.status.success());
}

// #[cfg(feature = "test-interactive")]
#[cfg(not(windows))]
#[test]
#[serial]
fn offline_flow_interactive() {
    use crate::util::{self, TestWorkspace};
    use expectrl::{ControlCode, Eof, Expect, Session};
    use insta::assert_snapshot;
    use std::{env, fs};

    util::apply_common_filters!();

    // setup mock server for dist_server from committed fixtures
    let s = util::mock_dist_server();

    // Set up project directory
    let ws = TestWorkspace::new();
    let project_path = ws.project_path();
    fs::create_dir_all(project_path).expect("should create project directory");
    env::set_current_dir(project_path).expect("should set current directory");

    // Install the toolchain interactively
    let mut cmd = ws.cli();
    cmd.env(constant::ENVNAME_MOONUP_DIST_SERVER, s.url())
        .arg("install")
        .arg("latest")
        .arg("--list-available");

    let mut p = Session::spawn(cmd).expect("should spawn moonup process");
    p.expect("Pick a version from latest channel")
        .expect("should prompt for selecting version");
    p.send_line("").expect("should send new line"); // select first version

    let output =
        String::from_utf8_lossy(p.expect(Eof).expect("should finish defaulting").as_bytes())
            .to_string();
    // Sanitize output to avoid snapshot mismatches
    let re = regex::Regex::new(r"(?s)Installing toolchain.*?Installed").unwrap();
    let output = re.replace_all(&output, "Installing toolchain...\n...\nInstalled");

    assert_snapshot!("moonup_install_latest_interactive", output);

    // Pin toolchain interactively
    let mut cmd = ws.cli();
    cmd.env(constant::ENVNAME_MOONUP_DIST_SERVER, s.url())
        .arg("pin");

    let mut p = Session::spawn(cmd).expect("should spawn moonup process");
    p.expect("Pick a installed version")
        .expect("should prompt for selecting installed version");
    p.send_line("").expect("should send new line"); // select first version

    let output = String::from_utf8_lossy(p.expect(Eof).expect("should finish pinning").as_bytes())
        .to_string();
    assert_snapshot!("moonup_pin_interactive", output);

    // Set default toolchain interactively
    let mut cmd = ws.cli();
    cmd.env(constant::ENVNAME_MOONUP_DIST_SERVER, s.url())
        .arg("default");

    let mut p = Session::spawn(cmd).expect("should spawn moonup process");
    p.expect("Pick a installed version")
        .expect("should prompt for selecting installed version");
    p.send_line("").expect("should send new line"); // select first version

    let output =
        String::from_utf8_lossy(p.expect(Eof).expect("should finish defaulting").as_bytes())
            .to_string();
    assert_snapshot!("moonup_default_interactive", output);

    // Uninstall the toolchain interactively
    let mut cmd = ws.cli();
    cmd.env(constant::ENVNAME_MOONUP_DIST_SERVER, s.url())
        .arg("uninstall");

    let mut p = Session::spawn(cmd).expect("should spawn moonup process");
    p.expect("Select toolchains to uninstall")
        .expect("should prompt for selecting installed version");
    p.send_line(ControlCode::Space) // select first version
        .expect("should send space");
    p.send_line("").expect("should send new line"); // confirm selection

    let output = String::from_utf8_lossy(
        p.expect(Eof)
            .expect("should finish uninstalling")
            .as_bytes(),
    )
    .to_string();
    assert_snapshot!("moonup_uninstall_interactive", output);

    env::set_current_dir(ws.tempdir()).expect("should restore current directory");
}

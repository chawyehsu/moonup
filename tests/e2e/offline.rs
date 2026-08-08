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
}

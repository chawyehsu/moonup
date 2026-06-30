use insta::glob;
use insta_cmd::assert_cmd_snapshot;
use mockito::Server;
use moonup::constant;
use serial_test::serial;
use std::{env, fs};

use crate::util::{self, TestWorkspace};

#[test]
#[serial]
fn test_basic_flow() {
    util::apply_common_filters!();

    let ws = TestWorkspace::new();

    assert_cmd_snapshot!("moonup_completions", ws.cli().arg("completions").arg("zsh"));

    // No toolchain installed
    assert_cmd_snapshot!("moonup_show", ws.cli().arg("show"));

    // Set default toolchain
    assert_cmd_snapshot!("moonup_default", ws.cli().arg("default").arg("latest"));

    // Set default toolchain interactively, but no toolchain installed,
    // should show subcommand help
    assert_cmd_snapshot!("moonup_default_2", ws.cli().arg("default"));

    // Pin toolchain
    let project_path = ws.project_path();
    fs::create_dir_all(project_path).expect("should create project directory");
    env::set_current_dir(project_path).expect("should set current directory");

    assert_cmd_snapshot!("moonup_pin", ws.cli().arg("pin").arg("nightly"));
    assert!(project_path.join(constant::TOOLCHAIN_FILE).exists());

    // Pin, but no toolchain installed, should show subcommand help
    assert_cmd_snapshot!("moonup_pin_2", ws.cli().arg("pin"));

    // Run command
    assert_cmd_snapshot!(
        "moonup_run_not_installed",
        ws.cli().arg("run").arg("nightly").arg("moon").arg("--help")
    );

    env::set_current_dir(ws.tempdir()).expect("should restore current directory");
}

#[test]
fn test_flow_with_network_mock() {
    util::apply_common_filters!();

    let ws = TestWorkspace::new();
    let mut s = Server::new();

    // setup mock server for dist_server
    // NOTE(chawyehsu): insta glob! macro does not support using one single path
    // (for example, `glob!("../fixtures/dist_server/**/*.json", |path: &Path| { ... })`)
    // to match files outside the current directory, a base path is required.
    glob!("../fixtures/dist_server", "**/*.json", |path: &Path| {
        let path = path.display().to_string();
        #[cfg(target_os = "windows")]
        let path = path.replace("\\", "/");

        let pathname = path.rsplit_once("dist_server").unwrap().1;
        // println!("Mocking: {} (fullpath: {})", pathname, path);

        s.mock("GET", pathname)
            .with_body_from_file(path)
            .with_header("content-type", "application/json")
            .create();
    });

    // Override the dist server URL with the mock server URL
    assert_cmd_snapshot!(
        "moonup_install_list_available_mock",
        ws.cli()
            .env(constant::ENVNAME_MOONUP_DIST_SERVER, s.url())
            .arg("install")
            .arg("--list-available")
    );
    // Should hit the cache and return the same result
    assert_cmd_snapshot!(
        "moonup_install_list_available_mock_2",
        ws.cli()
            .env(constant::ENVNAME_MOONUP_DIST_SERVER, s.url())
            .arg("install")
            .arg("--list-available")
            .arg("-vvv")
    );
    assert_cmd_snapshot!(
        "moonup_install_specific_version_unavailable_mock",
        ws.cli()
            .env(constant::ENVNAME_MOONUP_DIST_SERVER, s.url())
            .arg("install")
            .arg("0.1")
    );
}

/// Test flow with production networking
#[cfg(feature = "test-extra")]
mod liveinstall {
    use super::*;
    use serial_test::serial;

    #[cfg(feature = "test-liveinstall")]
    #[test]
    #[serial]
    fn test_flow_with_network() {
        util::apply_common_filters!();

        let ws = TestWorkspace::new();
        let test_install_version = "0.1.20241231+ba15a9a4e";
        let moon_exe_name = std::ffi::OsStr::new("moon");

        let build_moonup_path = ws.moonup();
        let build_output_path = build_moonup_path.parent().expect("should have parent");

        // Test self update (should be no update)
        assert_cmd_snapshot!("moonup_selfupdate", ws.cli().arg("self-update"));

        // Install a specific version of the toolchain
        assert_cmd_snapshot!(
            "moonup_install",
            ws.cli().arg("install").arg(test_install_version)
        );

        // List installed toolchains, installed toolchain should be listed
        assert_cmd_snapshot!("moonup_list_1", ws.cli().arg("list"));

        // Set up project directory
        let project_path = ws.project_path();
        fs::create_dir_all(project_path).expect("should create project directory");
        env::set_current_dir(project_path).expect("should set current directory");

        // Pin toolchain
        let pin_file = project_path.join(constant::TOOLCHAIN_FILE);
        assert_cmd_snapshot!("moonup_pin", ws.cli().arg("pin").arg(test_install_version));
        assert!(pin_file.exists());

        // Test command runner (runx)
        assert_cmd_snapshot!("moonup_which", ws.cli().arg("which").arg(moon_exe_name));
        assert_cmd_snapshot!(
            "moonup_run",
            ws.cli()
                .arg("run")
                .arg(test_install_version)
                .arg(moon_exe_name)
                .arg("version")
        );

        let moon_path = ws.moon_home().join("bin").display().to_string();
        let current_path = env::var("PATH").unwrap_or_else(|_| String::new());
        let updated_path = if current_path.is_empty() {
            moon_path
        } else {
            let env_separator = if cfg!(windows) { ";" } else { ":" };
            format!(
                "{}{}{}{}{}",
                moon_path,
                env_separator,
                build_output_path.display(),
                env_separator,
                current_path
            )
        };
        // println!("Updated PATH: {}", updated_path);

        // Test using pinned toolchain version
        temp_env::with_var("PATH", Some(updated_path.clone()), || {
            let mut cmd_moon = ws.cmd(moon_exe_name);
            assert_cmd_snapshot!(
                "moon_use_pinned_version",
                cmd_moon.arg("version").arg("--all")
            );

            let mut cmd_moon = ws.cmd(moon_exe_name);
            // No toolchain should be upgraded
            assert_cmd_snapshot!("moon_upgrade_intercept", cmd_moon.arg("upgrade"));
        });

        // Remove the pinned toolchain file
        std::fs::remove_file(pin_file).expect("should remove pinned toolchain file");

        // Test using specified toolchain version from argument (+ syntax)
        temp_env::with_var("PATH", Some(updated_path.clone()), || {
            // println!("PATH: {:?}", std::env::var("PATH"));
            let mut cmd_moon = ws.cmd(moon_exe_name);
            assert_cmd_snapshot!(
                "moon_use_version_from_arg",
                cmd_moon
                    .arg(format!("+{}", test_install_version))
                    .arg("version")
            );
        });

        // Uninstall the installed toolchain, keep cache
        assert_cmd_snapshot!(
            "moonup_uninstall_keep_cache",
            ws.cli()
                .arg("uninstall")
                .arg(test_install_version)
                .arg("--keep-cache")
        );

        // Cache should still exist
        let cache_path = ws
            .moonup_home()
            .join("downloads")
            .join("latest")
            .join(test_install_version);
        assert!(cache_path.exists());

        // Set default toolchain to a specific version
        assert_cmd_snapshot!(
            "moonup_default",
            ws.cli().arg("default").arg(test_install_version)
        );

        // Install the specific `default` version of the toolchain
        temp_env::with_var("PATH", Some(updated_path.clone()), || {
            let mut cmd_moon = ws.cmd(moon_exe_name);
            assert_cmd_snapshot!("moon_use_default_version", cmd_moon.arg("version"));
        });

        // List installed toolchains, no toolchain should be listed
        assert_cmd_snapshot!("moonup_list_2", ws.cli().arg("list"));

        // Remove cached files
        assert_cmd_snapshot!(
            "moonup_uninstall_clear",
            ws.cli().arg("uninstall").arg("--clear")
        );

        assert!(!cache_path.exists());

        // Uninstall the installed toolchain again, cache removed as well
        assert_cmd_snapshot!(
            "moonup_uninstall",
            ws.cli().arg("uninstall").arg(test_install_version)
        );

        // Create a custom bin file to test that it won't be removed by toolchain
        // update or reinstall
        let custom_bin = ws.moon_home().join("bin").join(if cfg!(windows) {
            "moonup-e2e-user-bin.cmd"
        } else {
            "moonup-e2e-user-bin"
        });
        fs::write(&custom_bin, "@echo off\n").expect("should create custom bin");

        // Toolchain should be uninstalled, cache should be removed
        let install_path = ws
            .moonup_home()
            .join("toolchains")
            .join(test_install_version);
        assert!(!install_path.exists());

        // List installed toolchains, no toolchain should be listed
        assert_cmd_snapshot!("moonup_list_3", ws.cli().arg("list"));

        // Test more toolchain specs
        assert_cmd_snapshot!(
            "moonup_install_neverexists",
            ws.cli().arg("install").arg("neverexists")
        );
        assert_cmd_snapshot!(
            "moonup_install_latest",
            ws.cli().arg("install").arg("latest")
        );
        assert_cmd_snapshot!(
            "moonup_install_nightly",
            ws.cli().arg("install").arg("nightly")
        );
        let test_nightly_version = "nightly-2026-01-31";
        assert_cmd_snapshot!(
            "moonup_install_nightly_2",
            ws.cli().arg("install").arg(test_nightly_version)
        );

        // LSP delegation tests
        // NOTE(chawyehsu): `moon-lsp` shim should be available after installing
        // the `latest` toolchain
        temp_env::with_var("PATH", Some(updated_path.clone()), || {
            let mut cmd_moonlsp = ws.cmd("moon-lsp");
            assert_cmd_snapshot!(
                "moon_lsp_nodebased_lsp_version",
                cmd_moonlsp
                    .arg(format!("+{}", test_nightly_version))
                    .arg("-v")
            );
        });
        // no javascript runtime
        let path = ws.moon_home().join("bin").display().to_string();
        temp_env::with_var("PATH", Some(path), || {
            let mut cmd_moonlsp = ws.cmd("moon-lsp");
            assert_cmd_snapshot!(
                "moon_lsp_nodebased_lsp_help",
                cmd_moonlsp
                    .arg(format!("+{}", test_nightly_version))
                    .arg("--help")
            );
        });

        // Test update toolchains
        assert_cmd_snapshot!("moonup_update", ws.cli().arg("update"));

        // Custom bin should still exist after install/update
        assert!(custom_bin.exists());

        // Test self update with forced update (for coverage)
        // This test is placed at the end because it will replace the currently running
        // executable.
        temp_env::with_var("MOONUP_TEST_FORCE_SELFUPDATE", Some("1"), || {
            assert_cmd_snapshot!("moonup_selfupdate_forced", ws.cli().arg("self-update"));
        });

        env::set_current_dir(ws.tempdir()).expect("should restore current directory");
    }

    #[cfg(feature = "test-interactive")]
    #[cfg(not(windows))]
    #[test]
    #[serial]
    fn test_flow_interactive() {
        use expectrl::{ControlCode, Eof, Expect, Session};
        use insta::assert_snapshot;

        util::apply_common_filters!();

        // Set up project directory
        let ws = TestWorkspace::new();
        let project_path = ws.project_path();
        fs::create_dir_all(project_path).expect("should create project directory");
        env::set_current_dir(project_path).expect("should set current directory");

        // Install toolchain interactively
        let mut cmd = ws.cli();
        cmd.arg("install").arg("latest").arg("--list-available");

        let mut p = Session::spawn(cmd).expect("should spawn moonup process");
        let duration = std::time::Duration::from_secs(60);
        p.set_expect_timeout(Some(duration));
        p.expect("Pick a version from latest channel")
            .expect("should prompt for selecting version");
        p.send_line("").expect("should send new line"); // select first version

        let output =
            String::from_utf8_lossy(p.expect(Eof).expect("should finish installing").as_bytes())
                .to_string();
        // Sanitize output to avoid snapshot mismatches
        let re = regex::Regex::new(r"(?s)Installing toolchain.*?Installed").unwrap();
        let output = re.replace_all(&output, "Installing toolchain...\n...\nInstalled");

        assert_snapshot!("moonup_install_interactive", output);

        // Pin toolchain interactively
        let mut cmd = ws.cli();
        cmd.arg("pin");

        let mut p = Session::spawn(cmd).expect("should spawn moonup process");
        p.expect("Pick a installed version")
            .expect("should prompt for selecting installed version");
        p.send_line("").expect("should send new line"); // select first version

        let output =
            String::from_utf8_lossy(p.expect(Eof).expect("should finish pinning").as_bytes())
                .to_string();
        assert_snapshot!("moonup_pin_interactive", output);

        // Set default toolchain interactively
        let mut cmd = ws.cli();
        cmd.arg("default");

        let mut p = Session::spawn(cmd).expect("should spawn moonup process");
        p.expect("Pick a installed version")
            .expect("should prompt for selecting installed version");
        p.send_line("").expect("should send new line"); // select first version

        let output =
            String::from_utf8_lossy(p.expect(Eof).expect("should finish defaulting").as_bytes())
                .to_string();
        assert_snapshot!("moonup_default_interactive", output);

        // Uninstall the installed toolchain interactively
        let mut cmd = ws.cli();
        cmd.arg("uninstall");

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
}

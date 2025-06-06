use insta::glob;
use insta_cmd::assert_cmd_snapshot;
use mockito::Server;
use moonup::constant;
use std::{env, fs};

use crate::util::{self, TestWorkspace};

#[test]
fn test_basic_flow() {
    util::apply_common_filters!();

    let ws = TestWorkspace::new();

    assert_cmd_snapshot!("moonup_completions", ws.cli().arg("completions").arg("zsh"));

    // No toolchain installed
    assert_cmd_snapshot!("moonup_show", ws.cli().arg("show"));

    // Set default toolchain
    assert_cmd_snapshot!("moonup_default", ws.cli().arg("default").arg("latest"));

    // Pin toolchain
    let project_path = ws.project_path();
    fs::create_dir_all(project_path).expect("should create project directory");
    env::set_current_dir(project_path).expect("should set current directory");

    assert_cmd_snapshot!("moonup_pin", ws.cli().arg("pin").arg("nightly"));
    assert!(project_path.join(constant::TOOLCHAIN_FILE).exists());

    env::set_current_dir(ws.tempdir()).expect("should restore current directory");
}

#[test]
fn test_flow_with_network_mock() {
    util::apply_common_filters!();

    let ws = TestWorkspace::new();
    let mut cli = ws.cli();

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
    assert_cmd_snapshot!(cli
        .env(constant::ENVNAME_MOONUP_DIST_SERVER, s.url())
        .arg("install")
        .arg("--list-available"));
}

/// Test flow with production networking
#[cfg(feature = "test-liveinstall")]
mod liveinstall {
    use super::*;

    #[test]
    fn test_flow_with_network() {
        util::apply_common_filters!();

        let ws = TestWorkspace::new();
        let test_install_version = "0.1.20241231+ba15a9a4e";
        let moon_exe_name = std::ffi::OsStr::new("moon");

        let build_moonup_path = insta_cmd::get_cargo_bin("moonup");
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

        // Pin toolchain
        let project_path = ws.project_path();
        let pin_file = project_path.join(constant::TOOLCHAIN_FILE);

        fs::create_dir_all(project_path).expect("should create project directory");
        env::set_current_dir(project_path).expect("should set current directory");

        assert_cmd_snapshot!("moonup_pin", ws.cli().arg("pin").arg(test_install_version));
        assert!(pin_file.exists());

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

        // Remove the pinned toolchain
        std::fs::remove_file(pin_file).expect("should remove pinned toolchain file");

        temp_env::with_var("PATH", Some(updated_path.clone()), || {
            println!("PATH: {:?}", std::env::var("PATH"));

            let mut cmd_moon = ws.cmd(moon_exe_name);
            assert_cmd_snapshot!(
                "moon_use_version_from_arg",
                cmd_moon
                    .arg(format!("+{}", test_install_version))
                    .arg("version")
            );
        });

        // Uninstall the installed toolchain
        assert_cmd_snapshot!(
            "moonup_uninstall_keep_cache",
            ws.cli()
                .arg("uninstall")
                .arg(test_install_version)
                .arg("--keep-cache")
        );

        let cache_path = ws
            .moonup_home()
            .join("downloads")
            .join("latest")
            .join(test_install_version);
        assert!(cache_path.exists());

        // Install the same version again, without specifying the version argument
        // assert_cmd_snapshot!("install_2", ws.cli().arg("install"));

        // Set default toolchain
        assert_cmd_snapshot!(
            "moonup_default",
            ws.cli().arg("default").arg(test_install_version)
        );

        temp_env::with_var("PATH", Some(updated_path), || {
            let mut cmd_moon = ws.cmd(moon_exe_name);
            assert_cmd_snapshot!("moon_use_default_version", cmd_moon.arg("version"));
        });

        // Uninstall the installed toolchain again, remove cache as well
        assert_cmd_snapshot!(
            "moonup_uninstall",
            ws.cli().arg("uninstall").arg(test_install_version)
        );

        // Toolchain should be uninstalled, cache should be removed
        let install_path = ws
            .moonup_home()
            .join("toolchains")
            .join(test_install_version);

        assert!(!install_path.exists());
        assert!(!cache_path.exists());

        // List installed toolchains, no toolchain should be listed
        assert_cmd_snapshot!("moonup_list_2", ws.cli().arg("list"));

        // Test more installations
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
        assert_cmd_snapshot!(
            "moonup_install_nightly_2",
            ws.cli().arg("install").arg("nightly-2025-05-21")
        );

        env::set_current_dir(ws.tempdir()).expect("should restore current directory");
    }
}

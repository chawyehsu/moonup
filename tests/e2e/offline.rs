//! End-to-end tests that exercise the mock dist server (committed fixtures).
//!
//! `online.rs` holds tests that involve the production dist server; the tests
//! here run against `mock_dist_server()` with zero network.

use std::{env, fs, path::Path};

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

#[cfg(not(windows))]
#[test]
fn moonup_shim_unix_sigterm() {
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

    let exitstatus = ws
        .cmd("moon")
        .env(constant::ENVNAME_MOONUP_DIST_SERVER, s.url())
        .arg("sigterm")
        .status()
        .expect("should run moon subcommand and get sigterm exit code");
    assert!(!exitstatus.success());
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

#[cfg(feature = "test-interactive")]
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

#[test]
fn moonup_install_unknown_channel() {
    util::apply_common_filters!();
    let ws = TestWorkspace::new();
    let s = util::mock_dist_server();

    // The spec resolves to a channel that the index does not advertise
    assert_cmd_snapshot!(
        "moonup_install_unknown_channel",
        ws.cli()
            .env(constant::ENVNAME_MOONUP_DIST_SERVER, s.url())
            .arg("install")
            .arg("nonexistent")
    );
}

#[test]
fn moonup_install_no_matching_release() {
    util::apply_common_filters!();
    let ws = TestWorkspace::new();
    let s = util::mock_dist_server();

    // The channel exists but no release matches the stable selector
    assert_cmd_snapshot!(
        "moonup_install_no_matching_release",
        ws.cli()
            .env(constant::ENVNAME_MOONUP_DIST_SERVER, s.url())
            .arg("install")
            .arg("9.9.9")
    );
}

#[test]
fn moonup_install_list_available_unknown_channel() {
    util::apply_common_filters!();
    let ws = TestWorkspace::new();
    let s = util::mock_dist_server();

    assert_cmd_snapshot!(
        "moonup_install_list_available_unknown_channel",
        ws.cli()
            .env(constant::ENVNAME_MOONUP_DIST_SERVER, s.url())
            .arg("install")
            .arg("nonexistent")
            .arg("--list-available")
    );
}

#[test]
fn moonup_update_up_to_date() {
    util::apply_common_filters!();
    let ws = TestWorkspace::new();
    let s = util::mock_dist_server();

    let _ = ws
        .cli()
        .env(constant::ENVNAME_MOONUP_DIST_SERVER, s.url())
        .arg("install")
        .arg("latest")
        .output()
        .expect("should install latest toolchain");

    assert_cmd_snapshot!(
        "moonup_update_up_to_date",
        ws.cli()
            .env(constant::ENVNAME_MOONUP_DIST_SERVER, s.url())
            .arg("update")
    );
}

#[test]
fn moonup_update_installs_newer_release() {
    use moonup::dist_server::schema::Target;

    util::apply_common_filters!();
    let ws = TestWorkspace::new();
    let version = crate::constant::E2E_TEST_MOCK_INSTALL_VERSION;
    let newer = "0.10.18+newer";
    let host = Target::from_host().expect("supported host").to_string();
    let targets = r#""aarch64-apple-darwin","aarch64-unknown-linux","x86_64-unknown-linux","x86_64-pc-windows""#;

    // The committed fixtures serve the initial install; afterwards the server
    // is reset and republished with a newer release added to the channel
    let mut server = util::mock_dist_server();

    let output = ws
        .cli()
        .env(constant::ENVNAME_MOONUP_DIST_SERVER, server.url())
        .arg("install")
        .arg("latest")
        .output()
        .expect("should run moonup install");
    assert!(output.status.success(), "install should succeed");

    // Invalidate the cached channel index so the update observes the new one
    let _ = ws
        .cli()
        .arg("uninstall")
        .arg("--clear")
        .output()
        .expect("should clear cached downloads");

    // Republish: same releases plus a newer one, reusing the same component
    // index and archive contents
    server.reset();
    server
        .mock("GET", "/index.json")
        .with_body_from_file(util::fixture_path("index.json"))
        .create();
    server
        .mock("GET", "/channel-latest.json")
        .with_body(format!(
            r#"{{"version":3,"lastModified":"20261121T0000000000Z","releases":[
                {{"version":"0.10.16+b1a0e846e","targets":[{targets}]}},
                {{"version":"{version}","targets":[{targets}]}},
                {{"version":"{newer}","targets":[{targets}]}}]}}"#
        ))
        .create();
    server
        .mock("GET", format!("/latest/{newer}/{host}.json").as_str())
        .with_body_from_file(util::fixture_path(&format!("latest/{version}/{host}.json")))
        .create();
    let archive_file = if host == "x86_64-pc-windows" {
        format!("moonbit-v{version}-{host}.zip")
    } else {
        format!("moonbit-v{version}-{host}.tar.gz")
    };
    server
        .mock("GET", format!("/download/v{newer}/{archive_file}").as_str())
        .with_body_from_file(util::fixture_path(&format!(
            "download/v{version}/{archive_file}"
        )))
        .create();

    assert_cmd_snapshot!(
        "moonup_update_installs_newer_release",
        ws.cli()
            .env(constant::ENVNAME_MOONUP_DIST_SERVER, server.url())
            .arg("update")
    );

    // The install metadata stub now records the newer release key
    let meta = fs::read_to_string(ws.moonup_home().join("toolchains/latest/version"))
        .expect("should read metadata stub");
    assert!(
        meta.contains(newer),
        "the stub should record the newer release key"
    );
}

#[test]
fn moonup_update_skips_incomplete_staging() {
    util::apply_common_filters!();
    let ws = TestWorkspace::new();
    let s = util::mock_dist_server();

    // An incomplete staging leftover (no completeness marker) is discovered
    // by `update`, but cannot be promoted: the toolchain is reported as not
    // installed instead of being skipped silently.
    std::fs::create_dir_all(
        ws.moonup_home()
            .join("toolchains")
            .join(".staging")
            .join("latest.new"),
    )
    .expect("should create incomplete staging leftover");

    assert_cmd_snapshot!(
        "moonup_update_skips_incomplete_staging",
        ws.cli()
            .env(constant::ENVNAME_MOONUP_DIST_SERVER, s.url())
            .arg("update")
    );
}

#[cfg(not(windows))]
fn craft_archive_with_internal_bin(src: &Path) -> Vec<u8> {
    use flate2::{Compression, read::GzDecoder, write::GzEncoder};
    use std::io::Read;
    use tar::{Archive, Builder, EntryType, Header};

    let tar_gz = fs::read(src).expect("should read stub archive");
    let mut archive = Archive::new(GzDecoder::new(tar_gz.as_slice()));
    let mut builder = Builder::new(GzEncoder::new(Vec::new(), Compression::default()));

    let mut moon_bytes: Option<Vec<u8>> = None;
    for entry in archive.entries().expect("should read archive entries") {
        let mut entry = entry.expect("should read archive entry");
        let path = entry.path().expect("entry path").to_path_buf();
        let mut data = Vec::new();
        entry
            .read_to_end(&mut data)
            .expect("should read entry data");
        if path == Path::new("bin/moon") {
            moon_bytes = Some(data.clone());
        }
        builder
            .append_data(&mut entry.header().clone(), &path, data.as_slice())
            .expect("should append archive entry");
    }
    let moon_bytes = moon_bytes.expect("stub archive should contain bin/moon");

    let mut header = Header::new_gnu();
    header.set_size(moon_bytes.len() as u64);
    header.set_mode(0o755);
    header.set_entry_type(EntryType::Regular);
    header.set_cksum();
    builder
        .append_data(
            &mut header,
            "bin/internal/moon-internal",
            moon_bytes.as_slice(),
        )
        .expect("should append internal bin");

    builder
        .into_inner()
        .expect("should finish tar")
        .finish()
        .expect("should finish gzip")
}

#[cfg(not(windows))]
#[test]
fn moonup_install_moonx_unavailable_and_internal_shims() {
    use moonup::dist_server::schema::Target;

    util::apply_common_filters!();
    let ws = TestWorkspace::new();
    let version = "0.10.17+internal";
    let host = Target::from_host().expect("supported host").to_string();

    let archive = craft_archive_with_internal_bin(Path::new(&util::fixture_path(&format!(
        "download/v0.10.17+d971670cd/moonbit-v0.10.17+d971670cd-{host}.tar.gz"
    ))));
    let sha256 =
        hex::encode(rattler_digest::compute_bytes_digest::<rattler_digest::Sha256>(&archive));

    // A release that declares moonx unavailable, requires --source-dir
    // bundling, and ships an internal binary.
    let mut server = util::mock_dist_server();
    server.reset();
    server
        .mock("GET", "/index.json")
        .with_body_from_file(util::fixture_path("index.json"))
        .create();
    server
        .mock("GET", "/channel-latest.json")
        .with_body(format!(
            r#"{{"version":3,"lastModified":"20261121T0000000000Z","releases":[{{"version":"{version}","moonx":"unavailable","bundleSourceDir":true}}]}}"#
        ))
        .create();
    server
        .mock(
            "GET",
            format!("/latest/{version}/{host}.json").as_str(),
        )
        .with_body(format!(
            r#"{{"version":2,"components":[{{"name":"toolchain","file":"moonbit-internal.tar.gz","sha256":"{sha256}"}}]}}"#
        ))
        .create();
    server
        .mock(
            "GET",
            format!("/download/v{version}/moonbit-internal.tar.gz").as_str(),
        )
        .with_body(archive)
        .create();

    let output = ws
        .cli()
        .env(constant::ENVNAME_MOONUP_DIST_SERVER, server.url())
        .arg("install")
        .arg(version)
        .output()
        .expect("should run moonup install");
    assert!(
        output.status.success(),
        "install should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let bin_dir = ws.moon_home().join("bin");
    let exe_suffix = std::env::consts::EXE_SUFFIX;

    assert!(
        bin_dir.join(format!("moon{exe_suffix}")).exists(),
        "the moon shim should be poured"
    );
    assert!(
        !bin_dir.join(format!("moonx{exe_suffix}")).exists(),
        "no moonx shim should be poured when the release marks moonx unavailable"
    );
    assert!(
        bin_dir
            .join("internal")
            .join(format!("moon-internal{exe_suffix}"))
            .exists(),
        "the internal binary should be poured into MOON_HOME/bin/internal"
    );
}

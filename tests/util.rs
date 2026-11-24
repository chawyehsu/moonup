use assert_fs::TempDir;
use mockito::ServerGuard;
use moonup::constant;
use moonup::toolchain::{InstallMeta, ToolchainSpec, atomic};
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;

/// A test workspace for e2e tests
#[allow(unused)]
pub struct TestWorkspace {
    /// Temporary directory root for the test workspace
    tempdir: TempDir,

    /// Custom MoonUp home directory in the tempdir path
    moonup_home: PathBuf,

    /// Custom MoonBit home directory in the tempdir path
    moon_home: PathBuf,

    /// Test project path
    project_path: PathBuf,
}

impl Default for TestWorkspace {
    fn default() -> Self {
        Self::new()
    }
}

impl TestWorkspace {
    /// Create a new test workspace
    pub fn new() -> Self {
        let tempdir = assert_fs::TempDir::new().expect("should create tempdir");
        let moonup_home = tempdir.path().join(".moonup");
        let moon_home = tempdir.path().join(".moon");
        let project_path = tempdir.path().join("my_moonbit_project");

        // FIXME: can we avoid this?
        std::fs::create_dir_all(&project_path).expect("should create project directory");

        Self {
            tempdir,
            moonup_home,
            moon_home,
            project_path,
        }
    }

    /// Run the given command with the test workspace environment
    ///
    /// `ws.bin()` will be used to resolve the path for the given command.
    pub fn cmd<S>(&self, cmd: S) -> Command
    where
        S: AsRef<OsStr>,
    {
        let mut cli = Command::new(self.bin(cmd));
        cli.env(constant::ENVNAME_MOONUP_HOME, self.moonup_home.as_os_str());
        cli.env(constant::ENVNAME_MOON_HOME, self.moon_home.as_os_str());

        cli.env("PATH", self.path_env());

        // Force emoji support on Windows during tests by setting WT_SESSION
        // see:
        //   https://github.com/console-rs/console/issues/207
        //   https://github.com/microsoft/terminal/issues/1040
        #[cfg(windows)]
        {
            cli.env("WT_SESSION", "1");
        }

        // Force emoji support on non-macOS Unix during tests by setting LANG,
        // as the `console` crate checks `LANG` to decide whether to use emoji.
        // see:
        //   https://github.com/console-rs/console/blob/0.16.3/src/unix_term.rs#L389-L396
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            cli.env("LANG", "en_US.UTF-8");
        }

        cli.current_dir(self.project_path.as_path());
        cli
    }

    /// Get the MoonUp CLI command
    pub fn cli(&self) -> Command {
        // Use a test distribution server that serves a fixed version of release
        // index for testing, so that we can ensure the same version is used in tests.
        static TEST_DIST_SERVER: &str = "https://moonup.csu.moe/testing/v3";

        let mut cmd = self.cmd("moonup");
        cmd.env(constant::ENVNAME_MOONUP_DIST_SERVER, TEST_DIST_SERVER);
        cmd
    }

    /// Get the MoonUp binary path in this test workspace
    pub fn moonup(&self) -> PathBuf {
        insta_cmd::get_cargo_bin("moonup")
    }

    /// Get the MoonBit home path
    #[allow(unused)]
    pub fn moon_home(&self) -> &Path {
        self.moon_home.as_path()
    }

    /// Get the MoonUp home path
    #[allow(unused)]
    pub fn moonup_home(&self) -> &Path {
        self.moonup_home.as_path()
    }

    /// Get shim path of given toolchain binary in this test workspace
    pub fn bin<S>(&self, name: S) -> PathBuf
    where
        S: AsRef<OsStr>,
    {
        let name = name.as_ref().to_str().expect("should convert OsStr to str");
        if name == "moonup" {
            return self.moonup();
        }

        let bin_dir = self.moon_home().join("bin");
        let path = if cfg!(windows) {
            bin_dir.join(format!("{name}.exe"))
        } else {
            bin_dir.join(name)
        };

        println!("Binary path: {}", path.display());

        path
    }

    /// Get the test workspace path
    #[allow(unused)]
    pub fn tempdir(&self) -> &TempDir {
        &self.tempdir
    }

    /// Get the test project path
    #[allow(unused)]
    pub fn project_path(&self) -> &Path {
        self.project_path.as_path()
    }

    /// PATH environment variable for the test workspace
    ///
    /// Includes the MoonUp binary directory and the Moon home bin directory.
    fn path_env(&self) -> OsString {
        let existing = std::env::var_os("PATH").unwrap_or_default();
        let mut paths = std::env::split_paths(&existing).collect::<Vec<_>>();

        paths.insert(0, self.moonup().parent().unwrap().to_path_buf());
        paths.insert(0, self.moon_home().join("bin"));
        std::env::join_paths(paths).expect("should join paths")
    }
}

/// Absolute path to a committed dist-server fixture file.
#[allow(unused)]
pub fn fixture_path(rel: &str) -> String {
    format!(
        "{}/tests/fixtures/dist_server/{}",
        env!("CARGO_MANIFEST_DIR"),
        rel
    )
}

/// Create a mockito server that serves every committed dist-server fixture.
///
/// The JSON component/channel/index files and the `.tar.gz` toolchain stub
/// archives under `tests/fixtures/dist_server` are served at their
/// `/download/...` URL paths, so install/update e2e tests run with zero
/// network. Each call returns a fresh owned server: tests that need dynamic
/// behavior can register shadowing mocks on it (mockito matches newer mocks
/// first) without affecting other tests running in parallel.
pub fn mock_dist_server() -> ServerGuard {
    let mut s = mockito::Server::new();
    const FIXTURE_DIR: &str = "fixtures/dist_server";

    // serve every committed JSON fixture (index, channel, component indexes)
    insta::glob!(FIXTURE_DIR, "**/*.json", |path: &Path| {
        let path = path.display().to_string();
        #[cfg(target_os = "windows")]
        let path = path.replace("\\", "/");

        let pathname = path.rsplit_once("dist_server").unwrap().1;
        s.mock("GET", pathname)
            .with_body_from_file(path)
            .with_header("content-type", "application/json")
            .create();
    });

    // serve the shrunken toolchain archives from disk
    insta::glob!(FIXTURE_DIR, "download/**/*.{tar.gz,zip}", |path: &Path| {
        let path = path.display().to_string();
        #[cfg(target_os = "windows")]
        let path = path.replace("\\", "/");

        let pathname = path.rsplit_once("dist_server").unwrap().1;
        s.mock("GET", pathname).with_body_from_file(path).create();
    });

    s
}

#[allow(unused_macros)]
macro_rules! apply_common_filters {
    {} => {
        let mut settings = insta::Settings::clone_current();
        // Remove moon build file lock prints
        settings.add_filter(r"Blocking waiting for file lock.*?\n", "");

        // Remove escape sequences
        settings.add_filter(r"\x1b(?:\[[0-9;?]*[A-Za-z]|\][^\x07]*\x07)", "");

        // moonup home
        settings.add_filter(r"~?(\b[A-Z]:)?[\\/](?:.*?[\\/])?\.moonup", "[MOONUP_HOME]");
        // moon home
        settings.add_filter(r"~?(\b[A-Z]:)?[\\/](?:.*?[\\/])?\.moon", "[MOON_HOME]");
        // moonup selfupdate
        settings.add_filter(r"Updating moonup: ([\d.]+) -> ([\d.]+)", "Updating moonup: [MOONUP_VERSION] -> [MOONUP_VERSION]");

        // Macos Temp Folder
        settings.add_filter(r"(/private)?/var/folders/\S+?/T/\S+", "[TEMP_FILE]");
        // Linux Temp Folder
        settings.add_filter(r"/tmp/\.tmp\S+", "[TEMP_FILE]");
        // Windows Temp folder
        settings.add_filter(r"\b[A-Z]:\\.*\\Local\\Temp\\\S+", "[TEMP_FILE]");
        // Convert windows paths to Unix Paths.
        settings.add_filter(r"\\\\?([\w\d.])", "/$1");
        // Remove Windows `.exe` suffix
        settings.add_filter(r"(moon.*)\.exe", "$1");
        let _bound = settings.bind_to_scope();
    }
}

#[allow(unused_imports)]
pub(crate) use apply_common_filters;

/// Fabricators for atomic-workflow crash states.
///
/// Every helper here requires `MOONUP_HOME` to point at the test home (call
/// inside `temp_env::with_var`). Together they are the single place in the
/// test suite that knows the on-disk atomic-workflow layout and the install
/// record format, so a format change updates this module and no test.
pub(crate) mod crash_state {
    use super::*;

    /// The install record JSON for the given spec and release key, carrying a
    /// finalization payload when one is given.
    #[allow(unused)]
    pub fn install_record(
        spec: &ToolchainSpec,
        key: &str,
        finalization: Option<serde_json::Value>,
    ) -> String {
        let mut record = serde_json::json!({
            "channel": spec.channel().as_str(),
            "key": key,
            "floating": matches!(spec, ToolchainSpec::Channel(_)),
        });
        if let Some(finalization) = finalization {
            record["finalization"] = finalization;
        }
        record.to_string()
    }

    /// The `.staging` directory under the current `MOONUP_HOME`.
    #[allow(unused)]
    pub fn staging_root() -> PathBuf {
        moonup::moonup_home()
            .join("toolchains")
            .join(atomic::STAGING_DIR)
    }

    /// Whether any retired leftover (`<name>.old` or
    /// `<name>.old.<pid>.<counter>`) exists for the spec.
    #[allow(unused)]
    pub fn has_retired_leftover(spec: &ToolchainSpec) -> bool {
        let prefix = format!("{}.old", spec.install_name());
        let Ok(read_dir) = staging_root().read_dir() else {
            return false;
        };
        read_dir.flatten().any(|e| {
            let name = e.file_name();
            let name = name.to_string_lossy();
            name.starts_with(&prefix)
        })
    }

    /// Whether the live install record for the spec carries a finalization
    /// payload.
    #[allow(unused)]
    pub fn live_record_has_finalization(spec: &ToolchainSpec) -> bool {
        let live = spec.install_path();
        let content = std::fs::read_to_string(live.join("version")).expect("should read record");
        serde_json::from_str::<InstallMeta>(&content)
            .expect("should parse install record")
            .finalization
            .is_some()
    }

    /// Fabricate a live install: the install directory with `bin/moon` content
    /// and the given install record.
    #[allow(unused)]
    pub fn write_live_install(spec: &ToolchainSpec, record: &str) -> PathBuf {
        let live = spec.install_path();
        std::fs::create_dir_all(live.join("bin")).expect("should create live dir");
        std::fs::write(live.join("bin").join("moon"), b"live").expect("should write live content");
        std::fs::write(live.join("version"), record).expect("should write install record");
        live
    }

    /// Fabricate a complete staging: `bin/moon` content plus an install record
    /// with the given finalization payload, as `populate` writes it before the
    /// swap it did not survive.
    #[allow(unused)]
    pub fn write_complete_staging(
        spec: &ToolchainSpec,
        key: &str,
        finalization: serde_json::Value,
    ) -> PathBuf {
        let staging = atomic::staging_dir_for(spec);
        std::fs::create_dir_all(staging.join("bin")).expect("should create staging dir");
        std::fs::write(staging.join("bin").join("moon"), b"new")
            .expect("should write staging content");
        let record = install_record(spec, key, Some(finalization));
        std::fs::write(staging.join("version"), record).expect("should write staged record");
        staging
    }

    /// Fabricate an incomplete staging: `bin/moon` content, no install record,
    /// as a crash during extraction leaves behind.
    #[allow(unused)]
    pub fn write_partial_staging(spec: &ToolchainSpec) -> PathBuf {
        let staging = atomic::staging_dir_for(spec);
        std::fs::create_dir_all(staging.join("bin")).expect("should create staging dir");
        std::fs::write(staging.join("bin").join("moon"), b"new")
            .expect("should write staging content");
        staging
    }

    /// Fabricate a retired leftover under the legacy deterministic name
    /// (`<name>.old`) that older moonup versions used.
    #[allow(unused)]
    pub fn write_legacy_retired(spec: &ToolchainSpec) -> PathBuf {
        let retired = staging_root().join(format!("{}.old", spec.install_name()));
        std::fs::create_dir_all(&retired).expect("should create legacy retired dir");
        retired
    }

    /// Fabricate a retired leftover under a unique pid-counter name
    /// (`<name>.old.<pid>.<counter>`).
    #[allow(unused)]
    pub fn write_unique_retired(spec: &ToolchainSpec, counter: u64) -> PathBuf {
        let retired = staging_root().join(format!(
            "{}.old.{}.{counter}",
            spec.install_name(),
            std::process::id()
        ));
        std::fs::create_dir_all(&retired).expect("should create retired leftover dir");
        retired
    }

    /// Fabricate the legacy completeness marker of older moonup versions
    /// (`<name>.complete`).
    #[allow(unused)]
    pub fn write_legacy_complete_marker(spec: &ToolchainSpec, version: &str) -> PathBuf {
        let marker = staging_root().join(format!("{}.complete", spec.install_name()));
        std::fs::create_dir_all(marker.parent().expect("should have .staging parent"))
            .expect("should create .staging dir");
        std::fs::write(&marker, format!(r#"{{"version":"{version}"}}"#))
            .expect("should write legacy marker");
        marker
    }
}

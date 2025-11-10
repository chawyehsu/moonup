use assert_fs::TempDir;
use moonup::constant;
use std::ffi::OsStr;
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
    pub fn cmd<S>(&self, cmd: S) -> Command
    where
        S: AsRef<OsStr>,
    {
        let mut cli = Command::new(cmd);
        cli.env(constant::ENVNAME_MOONUP_HOME, self.moonup_home.as_os_str());
        cli.env(constant::ENVNAME_MOON_HOME, self.moon_home.as_os_str());

        // Force emoji support on Windows during tests by setting WT_SESSION
        // see:
        //   https://github.com/console-rs/console/issues/207
        //   https://github.com/microsoft/terminal/issues/1040
        #[cfg(windows)]
        {
            cli.env("WT_SESSION", "1");
        }

        cli.current_dir(self.project_path.as_path());
        cli
    }

    /// Get the MoonUp CLI command
    pub fn cli(&self) -> Command {
        // Use a test distribution server that serves a fixed version of release
        // index for testing, so that we can ensure the same version is used in tests.
        static TEST_DIST_SERVER: &str = "https://moonup.csu.moe/testing/v2";

        let mut cmd = self.cmd(self.moonup().as_os_str());
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
        settings.add_filter(r"(\b[A-Z]:)?[\\/].*?[\\/].moonup", "[MOONUP_HOME]");
        // moon home
        settings.add_filter(r"~?(\b[A-Z]:)?[\\/].*?[\\/].moon", "[MOON_HOME]");
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

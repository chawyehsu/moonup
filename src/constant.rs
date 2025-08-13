/// The default Moon home directory is `~/.moon`
pub const MOON_DIR: &str = ".moon";

/// The default MoonUp home directory is `~/.moonup`
pub const MOONUP_DIR: &str = ".moonup";

/// The environment variable name for customizing MoonBit home directory
pub const ENVNAME_MOON_HOME: &str = "MOON_HOME";

/// The environment variable name for customizing MoonUp home directory
pub const ENVNAME_MOONUP_HOME: &str = "MOONUP_HOME";

/// The environment variable name for customizing the MoonUp distribution server
pub const ENVNAME_MOONUP_DIST_SERVER: &str = "MOONUP_DIST_SERVER";

/// The URL of the MoonUp distribution server
pub const MOONUP_DIST_SERVER: &str = "https://moonup.csu.moe/v2";

/// The expiration time for the release index, in hours
pub const INDEX_EXPIRATION: i64 = 1;

/// The timeout for reading HTTP responses, in seconds
pub const HTTP_READ_TIMEOUT: u64 = 5 * 60;

/// The filename for specifying the toolchain version
pub const TOOLCHAIN_FILE: &str = "moonbit-version";

/// The maximum number of recursions allowed
pub const RECURSION_LIMIT: u8 = 20;

/// The maximum number of select dialog items
pub const MAX_SELECT_ITEMS: usize = 6;

/// The allowed file extensions to be detected as executables on Windows
#[cfg(target_os = "windows")]
pub const ALLOWED_EXTENSIONS: [&str; 3] = ["exe", "bat", "cmd"];

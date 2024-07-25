/// The default Moon home directory is `~/.moon`
pub const MOON_DIR: &str = ".moon";

/// The default MoonUp home directory is `~/.moonup`
pub const MOONUP_DIR: &str = ".moonup";

/// The URL of the MoonBit release index
pub const INDEX_URL: &str = "https://chawyehsu.github.io/moonbit-binaries/index.json";

/// The expiration time for the release index, in hours
pub const INDEX_EXPIRATION: i64 = 2;

/// The timeout for reading HTTP responses, in seconds
pub const HTTP_READ_TIMEOUT: u64 = 5 * 60;

/// The filename for specifying the toolchain version
pub const TOOLCHAIN_FILE: &str = "moonbit-version";

/// The maximum number of recursions allowed
pub const RECURSION_LIMIT: u8 = 20;

use serde::Deserialize;

/// The main index of the distribution server
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Index {
    pub version: u8,
    pub last_modified: String,
    pub channels: Vec<Channel>,
    pub targets: Vec<Target>,
}

/// Represents a channel in the index
#[derive(Debug, Deserialize)]
pub struct Channel {
    /// The channel name
    pub name: ChannelName,
    /// The (compiler) version number of the latest release in the channel
    pub version: String,
    /// (Optional) The (nightly) build date of the latest release in the channel
    pub date: Option<String>,
}

impl std::fmt::Display for Channel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.version, self.name)
    }
}

/// Represents a component in the component index
#[derive(Debug, Clone, Deserialize)]
pub struct Component {
    /// The component name
    pub name: String,
    /// The component file name
    pub file: String,
    /// The sha256 checksum of the component
    pub sha256: String,
}

/// The component index
#[derive(Debug, Deserialize)]
pub struct ComponentIndex {
    pub version: u8,
    pub components: Vec<Component>,
}

/// Channel names
#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub enum ChannelName {
    /// The latest/stable channel
    Latest,

    /// The nightly channel
    Nightly,

    /// All other channel unsupported (yet) are treated as unknown
    #[serde(untagged)]
    Unknown(String),
}

impl std::fmt::Display for ChannelName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChannelName::Latest => write!(f, "latest"),
            ChannelName::Nightly => write!(f, "nightly"),
            ChannelName::Unknown(c) => write!(f, "{c}"),
        }
    }
}

/// The channel index
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelIndex {
    pub version: u8,
    pub last_modified: String,
    pub releases: Vec<Release>,
}

/// Represents a release in the channel index
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    /// The (compiler) version number of the release
    pub version: String,
    /// Flag to indicate if the distribution layout of the release is version 1
    pub layout_version1: Option<bool>,
    /// The (nightly build) date of the release
    pub date: Option<String>,
}

/// The target architecture of the toolchain
#[derive(Debug, Deserialize, Clone, PartialEq)]
#[non_exhaustive]
pub enum Target {
    #[serde(rename = "aarch64-apple-darwin")]
    Aarch64MacOS,
    #[serde(rename = "x86_64-apple-darwin")]
    Amd64MacOS,
    #[serde(rename = "x86_64-unknown-linux")]
    Amd64Linux,
    #[serde(rename = "x86_64-pc-windows")]
    Amd64Windows,
    /// All other targets unsupported (yet) are treated as unknown
    #[serde(untagged)]
    Unknown(String),
}

impl Target {
    pub fn from_host() -> miette::Result<Self> {
        match std::env::consts::OS {
            "macos" => match std::env::consts::ARCH {
                "aarch64" => Ok(Target::Aarch64MacOS),
                "x86_64" => Ok(Target::Amd64MacOS),
                _ => Err(miette::miette!("unsupported architecture")),
            },
            "linux" => Ok(Target::Amd64Linux),
            "windows" => Ok(Target::Amd64Windows),
            _ => Err(miette::miette!("unsupported platform")),
        }
    }
}

impl std::fmt::Display for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Target::Aarch64MacOS => write!(f, "aarch64-apple-darwin"),
            Target::Amd64MacOS => write!(f, "x86_64-apple-darwin"),
            Target::Amd64Linux => write!(f, "x86_64-unknown-linux"),
            Target::Amd64Windows => write!(f, "x86_64-pc-windows"),
            Target::Unknown(t) => write!(f, "{t}"),
        }
    }
}

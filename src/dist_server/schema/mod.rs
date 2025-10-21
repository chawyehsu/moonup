use serde::Deserialize;

mod legacy;
mod v2;
mod v3;

#[derive(Debug)]
pub enum VersionedIndex {
    /// Schema version 2
    V2(v2::Index),

    /// Schema version 3
    V3(v3::Index),

    /// Unsupported index version
    Unsupported,
}

// Workaround to https://github.com/serde-rs/serde/issues/745
impl<'de> serde::Deserialize<'de> for VersionedIndex {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(d)?;
        let version = value.get("version").and_then(serde_json::Value::as_u64);

        Ok(match version {
            Some(2) => VersionedIndex::V2(v2::Index::deserialize(value).unwrap()),
            Some(3) => VersionedIndex::V3(v3::Index::deserialize(value).unwrap()),
            _ => VersionedIndex::Unsupported,
        })
    }
}

/// The main index of the distribution server
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Index {
    /// Versioned index formats (v2, v3, ...)
    Versioned(VersionedIndex),

    /// Legacy index format (v1)
    Legacy(legacy::Index),

    /// Unknown index format
    Unsupported(serde_json::Value),
}

impl Index {
    /// Get the channels from the index if available
    pub fn channels(&self) -> &[Channel] {
        match self {
            Index::Versioned(VersionedIndex::V2(i)) => i.channels.as_slice(),
            Index::Versioned(VersionedIndex::V3(i)) => i.channels.as_slice(),
            _ => &[],
        }
    }
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
        match self.name {
            ChannelName::Bleeding | ChannelName::Latest => {
                write!(f, "{} ({})", self.name, self.version)
            }
            ChannelName::Nightly => {
                write!(
                    f,
                    "{} ({}, {})",
                    self.name,
                    self.version,
                    self.date.as_deref().unwrap_or("unknown")
                )
            }
            ChannelName::Unknown(_) => Ok(()),
        }
    }
}

/// Channel names
#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub enum ChannelName {
    /// The bleeding channel
    Bleeding,

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
            ChannelName::Bleeding => write!(f, "bleeding"),
            ChannelName::Latest => write!(f, "latest"),
            ChannelName::Nightly => write!(f, "nightly"),
            ChannelName::Unknown(c) => write!(f, "{c}"),
        }
    }
}

#[derive(Debug)]
pub enum VersionedComponentIndex {
    /// channel index version 2
    V2(v2::ComponentIndex),

    /// Unsupported channel index version
    Unsupported,
}

// Workaround to https://github.com/serde-rs/serde/issues/745
impl<'de> serde::Deserialize<'de> for VersionedComponentIndex {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(d)?;
        let version = value.get("version").and_then(serde_json::Value::as_u64);

        Ok(match version {
            Some(2) => VersionedComponentIndex::V2(v2::ComponentIndex::deserialize(value).unwrap()),
            _ => VersionedComponentIndex::Unsupported,
        })
    }
}

/// The component index
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum ComponentIndex {
    /// Versioned component index formats (v2, v3, ...)
    Versioned(VersionedComponentIndex),

    /// Unknown component index format
    Unsupported(serde_json::Value),
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

impl ComponentIndex {
    /// Get the components from the component index if available
    pub fn components(&self) -> &[Component] {
        match self {
            ComponentIndex::Versioned(VersionedComponentIndex::V2(i)) => i.components.as_slice(),
            ComponentIndex::Unsupported(_) => &[],
            _ => &[],
        }
    }
}

#[derive(Debug)]
pub enum VersionedChannelIndex {
    /// channel index version 2
    V2(v2::ChannelIndex),

    /// channel index version 3
    V3(v3::ChannelIndex),

    /// Unsupported channel index version
    Unsupported,
}

// Workaround to https://github.com/serde-rs/serde/issues/745
impl<'de> serde::Deserialize<'de> for VersionedChannelIndex {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(d)?;
        let version = value.get("version").and_then(serde_json::Value::as_u64);

        Ok(match version {
            Some(2) => VersionedChannelIndex::V2(v2::ChannelIndex::deserialize(value).unwrap()),
            Some(3) => VersionedChannelIndex::V3(v3::ChannelIndex::deserialize(value).unwrap()),
            _ => VersionedChannelIndex::Unsupported,
        })
    }
}

/// The channel index
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum ChannelIndex {
    /// Versioned index formats (v2, v3, ...)
    Versioned(VersionedChannelIndex),

    /// Unknown index format
    Unsupported(serde_json::Value),
}

impl ChannelIndex {
    /// Get the releases from the channel index if available
    pub fn releases(&self) -> &[Release] {
        match self {
            ChannelIndex::Versioned(VersionedChannelIndex::V2(i)) => i.releases.as_slice(),
            ChannelIndex::Versioned(VersionedChannelIndex::V3(i)) => i.releases.as_slice(),
            ChannelIndex::Unsupported(_) => &[],
            _ => &[],
        }
    }
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

    /// The available target hosts for the release
    pub targets: Option<Vec<Target>>,
}

impl Release {
    /// Check if the current host is supported by this release
    pub fn is_host_supported(&self) -> bool {
        let host = match Target::from_host() {
            Ok(h) => h,
            Err(_) => {
                // If cannot determine host, assume unsupported
                return false;
            }
        };

        let targets = match &self.targets {
            Some(t) => t,
            None => {
                // If no targets are specified in the release, assume supported
                return true;
            }
        };

        targets.contains(&host)
    }
}

/// The target architecture of the toolchain
#[derive(Debug, Deserialize, Clone, PartialEq)]
#[non_exhaustive]
pub enum Target {
    /// Apple macOS on ARM64 (Apple Silicon)
    #[serde(rename = "aarch64-apple-darwin")]
    Aarch64MacOS,

    /// Apple macOS on AMD64 (Intel)
    #[serde(rename = "x86_64-apple-darwin")]
    Amd64MacOS,

    /// Linux on AMD64
    #[serde(rename = "x86_64-unknown-linux")]
    Amd64Linux,

    /// Windows on AMD64
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

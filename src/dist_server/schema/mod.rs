use jiff::civil::Date;
use serde::{Deserialize, Serialize, de::Error};

mod legacy;
mod v2;
mod v3;

/// A syntactically valid dist-server channel name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChannelName(String);

impl ChannelName {
    /// Parse a channel name containing only lowercase ASCII letters, digits,
    /// and hyphens.
    pub fn parse(name: &str) -> Result<Self, InvalidChannelName> {
        Self::try_from(name.to_owned())
    }

    /// Borrow the channel name as a string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<&str> for ChannelName {
    type Error = InvalidChannelName;

    fn try_from(name: &str) -> Result<Self, Self::Error> {
        Self::parse(name)
    }
}

impl TryFrom<String> for ChannelName {
    type Error = InvalidChannelName;

    fn try_from(name: String) -> Result<Self, Self::Error> {
        if !name.is_empty()
            && name
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        {
            Ok(Self(name))
        } else {
            Err(InvalidChannelName { channel: name })
        }
    }
}

impl AsRef<str> for ChannelName {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::fmt::Display for ChannelName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for ChannelName {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for ChannelName {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let name = <String as serde::Deserialize>::deserialize(deserializer)?;
        Self::try_from(name).map_err(D::Error::custom)
    }
}

/// A string that does not match the dist-server channel-name grammar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidChannelName {
    channel: String,
}

impl std::fmt::Display for InvalidChannelName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid channel name '{}'", self.channel)
    }
}

impl std::error::Error for InvalidChannelName {}

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

        match version {
            None => Err(D::Error::custom("missing version field in versioned index")),
            Some(2) => Ok(VersionedIndex::V2(
                v2::Index::deserialize(value).map_err(D::Error::custom)?,
            )),
            Some(3) => Ok(VersionedIndex::V3(
                v3::Index::deserialize(value).map_err(D::Error::custom)?,
            )),
            _ => Ok(VersionedIndex::Unsupported),
        }
    }
}

/// The main index of the distribution server
#[derive(Debug)]
pub enum Index {
    /// Versioned index formats (v2, v3, ...)
    Versioned(VersionedIndex),

    /// Legacy index format (v1)
    Legacy(legacy::Index),

    /// Unknown index format
    Unsupported(serde_json::Value),
}

impl<'de> serde::Deserialize<'de> for Index {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(d)?;
        if value
            .get("version")
            .and_then(serde_json::Value::as_u64)
            .is_some()
        {
            return VersionedIndex::deserialize(value)
                .map(Index::Versioned)
                .map_err(D::Error::custom);
        }

        match legacy::Index::deserialize(value.clone()) {
            Ok(index) => Ok(Index::Legacy(index)),
            Err(_) => Ok(Index::Unsupported(value)),
        }
    }
}

impl Index {
    /// Get the channels from the index. Names are validated during deserialization.
    pub fn channels(&self) -> Vec<&Channel> {
        match self {
            Index::Versioned(VersionedIndex::V2(i)) => i.channels.iter().collect(),
            Index::Versioned(VersionedIndex::V3(i)) => i.channels.iter().collect(),
            _ => Vec::new(),
        }
    }
}

/// A build date in the dist-server schema, formatted as `YYYY-MM-DD`.
///
/// The underlying date implementation is intentionally private. Use [`BuildDate::parse`]
/// to construct a value from its wire representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BuildDate(Date);

impl BuildDate {
    /// Parse a valid build date in the exact `YYYY-MM-DD` format.
    pub fn parse(value: &str) -> Option<Self> {
        parse_build_date(value).map(Self)
    }
}

impl std::fmt::Display for BuildDate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Serialize for BuildDate {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> serde::Deserialize<'de> for BuildDate {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).ok_or_else(|| {
            D::Error::custom(format!("invalid build date {value:?}; expected YYYY-MM-DD"))
        })
    }
}

fn parse_build_date(value: &str) -> Option<Date> {
    let bytes = value.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || !bytes[..4].iter().all(u8::is_ascii_digit)
        || !bytes[5..7].iter().all(u8::is_ascii_digit)
        || !bytes[8..].iter().all(u8::is_ascii_digit)
    {
        return None;
    }

    Date::new(
        value[..4].parse().ok()?,
        value[5..7].parse().ok()?,
        value[8..].parse().ok()?,
    )
    .ok()
}

/// Represents a channel in the index
#[derive(Debug, Deserialize)]
pub struct Channel {
    /// The channel name, matching `[a-z0-9-]+`.
    pub name: ChannelName,
    /// The (compiler) version number of the latest release in the channel
    pub version: String,
    /// Optional nightly build date of the latest release, formatted as `YYYY-MM-DD`.
    #[serde(default)]
    pub date: Option<BuildDate>,
}

impl std::fmt::Display for Channel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.date.as_ref() {
            Some(date) => write!(f, "{} ({}, {})", self.name, self.version, date),
            None => write!(f, "{} ({})", self.name, self.version),
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

        match version {
            None => Err(D::Error::custom(
                "missing version field in versioned component index",
            )),
            Some(2) => Ok(VersionedComponentIndex::V2(
                v2::ComponentIndex::deserialize(value).unwrap(),
            )),
            _ => Ok(VersionedComponentIndex::Unsupported),
        }
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

        match version {
            None => Err(D::Error::custom(
                "missing version field in versioned channel index",
            )),
            Some(2) => Ok(VersionedChannelIndex::V2(
                v2::ChannelIndex::deserialize(value).map_err(D::Error::custom)?,
            )),
            Some(3) => Ok(VersionedChannelIndex::V3(
                v3::ChannelIndex::deserialize(value).map_err(D::Error::custom)?,
            )),
            _ => Ok(VersionedChannelIndex::Unsupported),
        }
    }
}

/// The channel index
#[derive(Debug)]
pub enum ChannelIndex {
    /// Versioned index formats (v2, v3, ...)
    Versioned(VersionedChannelIndex),

    /// Unknown index format
    Unsupported(serde_json::Value),
}

impl<'de> serde::Deserialize<'de> for ChannelIndex {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(d)?;
        if value
            .get("version")
            .and_then(serde_json::Value::as_u64)
            .is_some()
        {
            return VersionedChannelIndex::deserialize(value)
                .map(ChannelIndex::Versioned)
                .map_err(D::Error::custom);
        }
        Ok(ChannelIndex::Unsupported(value))
    }
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    /// The (compiler) version number of the release
    pub version: String,

    /// Flag to indicate if the distribution layout of the release is version 1
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout_version1: Option<bool>,

    /// Flag to indicate if `--source-dir` should be used for running `moon bundle`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bundle_source_dir: Option<bool>,

    /// Flag to indicate the support for `moonx` in this release, undefined
    /// means that it is assumed to be supported
    ///
    /// Possible values:
    /// - `"unavailable"`: `moonx` is not supported in this release
    #[serde(skip_serializing_if = "Option::is_none")]
    pub moonx: Option<String>,

    /// Optional nightly build date of the release, formatted as `YYYY-MM-DD`.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub date: Option<BuildDate>,

    /// The available target hosts for the release
    #[serde(skip_serializing_if = "Option::is_none")]
    pub targets: Option<Vec<Target>>,
}

impl Release {
    /// Create a new release with the given version.
    pub fn new(version: impl Into<String>) -> Self {
        Self {
            version: version.into(),
            layout_version1: None,
            bundle_source_dir: None,
            moonx: None,
            date: None,
            targets: None,
        }
    }

    /// The path segment identifying this release within its channel: the
    /// build date when present, otherwise the version.
    pub fn key(&self) -> String {
        self.date
            .map_or_else(|| self.version.clone(), |date| date.to_string())
    }

    /// Check if the current host is supported by this release
    pub fn is_host_supported(&self) -> bool {
        match Target::from_host() {
            Ok(host) => self.is_target_supported(&host),
            // If cannot determine host, assume unsupported
            Err(_) => false,
        }
    }

    /// Check if a specific target host is supported by this release
    pub fn is_target_supported(&self, host: &Target) -> bool {
        match &self.targets {
            Some(targets) => targets.contains(host),
            // If no targets are specified in the release, assume supported
            None => true,
        }
    }
}

/// The target architecture of the toolchain
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[non_exhaustive]
pub enum Target {
    /// Apple macOS on ARM64 (Apple Silicon)
    #[serde(rename = "aarch64-apple-darwin")]
    Aarch64MacOS,

    /// Apple macOS on AMD64 (Intel)
    #[serde(rename = "x86_64-apple-darwin")]
    Amd64MacOS,

    /// Linux on ARM64
    #[serde(rename = "aarch64-unknown-linux")]
    Aarch64Linux,

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
    pub fn from(os: &str, arch: &str) -> miette::Result<Self> {
        match os {
            "macos" => match arch {
                "aarch64" => Ok(Target::Aarch64MacOS),
                "x86_64" => Ok(Target::Amd64MacOS),
                _ => Err(miette::miette!("unsupported architecture")),
            },
            "linux" => match arch {
                "aarch64" => Ok(Target::Aarch64Linux),
                "x86_64" => Ok(Target::Amd64Linux),
                _ => Err(miette::miette!("unsupported architecture")),
            },
            "windows" => Ok(Target::Amd64Windows),
            _ => Err(miette::miette!("unsupported platform")),
        }
    }

    #[inline]
    pub fn from_host() -> miette::Result<Self> {
        Target::from(std::env::consts::OS, std::env::consts::ARCH)
    }
}

impl std::fmt::Display for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Target::Aarch64MacOS => write!(f, "aarch64-apple-darwin"),
            Target::Amd64MacOS => write!(f, "x86_64-apple-darwin"),
            Target::Amd64Linux => write!(f, "x86_64-unknown-linux"),
            Target::Aarch64Linux => write!(f, "aarch64-unknown-linux"),
            Target::Amd64Windows => write!(f, "x86_64-pc-windows"),
            Target::Unknown(t) => write!(f, "{t}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_channel_names() {
        for name in ["latest", "dev-preview", "nightly-2", "123"] {
            assert!(
                ChannelName::parse(name).is_ok(),
                "expected {name:?} to be valid"
            );
        }

        for name in ["", "Latest", "dev_preview", "dev/preview", "dev@preview"] {
            assert!(
                ChannelName::parse(name).is_err(),
                "expected {name:?} to be invalid"
            );
        }
    }

    #[test]
    fn channel_name_serde_preserves_string_representation_and_validation() {
        let name = ChannelName::parse("dev-preview").expect("valid channel name");
        assert_eq!(name.to_string(), "dev-preview");
        assert_eq!(
            serde_json::to_string(&name).expect("serialize channel name"),
            r#""dev-preview""#
        );
        assert_eq!(
            serde_json::from_str::<ChannelName>(r#""dev-preview""#)
                .expect("deserialize channel name"),
            name
        );
        assert!(serde_json::from_str::<ChannelName>(r#""Dev_preview""#).is_err());
    }

    #[test]
    fn rejects_indexes_with_invalid_channel_names() {
        let index = serde_json::from_str::<Index>(
            r#"{
                "version": 2,
                "lastModified": "",
                "channels": [
                    {"name": "latest", "version": "1.0.0"},
                    {"name": "dev-preview", "version": "1.0.0-preview"},
                    {"name": "Invalid_Name", "version": "1.0.0"}
                ],
                "targets": []
            }"#,
        );

        assert!(index.is_err());
    }

    #[test]
    fn validates_channel_and_release_dates_during_deserialization() {
        let channel: Channel =
            serde_json::from_str(r#"{"name":"nightly","version":"1.0.0","date":"2024-02-29"}"#)
                .expect("valid channel date should deserialize");
        assert_eq!(channel.date.unwrap().to_string(), "2024-02-29");

        let release: Release = serde_json::from_str(r#"{"version":"1.0.0","date":"2024-02-29"}"#)
            .expect("valid release date should deserialize");
        assert_eq!(release.date.unwrap().to_string(), "2024-02-29");

        for date in ["2026-02-31", "2026-2-01", "2026-01-01T00:00:00Z"] {
            let channel = serde_json::json!({
                "name": "nightly",
                "version": "1.0.0",
                "date": date,
            });
            assert!(
                serde_json::from_value::<Channel>(channel).is_err(),
                "{date}"
            );

            let release = serde_json::json!({"version": "1.0.0", "date": date});
            assert!(
                serde_json::from_value::<Release>(release).is_err(),
                "{date}"
            );
        }

        let encoded = serde_json::to_value(release).expect("release should serialize");
        assert_eq!(encoded["date"], "2024-02-29");
    }

    #[test]
    fn rejects_invalid_dates_in_versioned_documents() {
        let index = serde_json::from_str::<Index>(
            r#"{
                "version": 3,
                "lastModified": "",
                "channels": [
                    {"name": "nightly", "version": "1.0.0", "date": "2026-02-31"}
                ]
            }"#,
        );
        assert!(index.is_err());

        let channel_index = serde_json::from_str::<ChannelIndex>(
            r#"{
                "version": 3,
                "lastModified": "",
                "releases": [
                    {"version": "1.0.0", "date": "2026-02-31"}
                ]
            }"#,
        );
        assert!(channel_index.is_err());
    }
}

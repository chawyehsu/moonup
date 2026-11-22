use chrono::NaiveDate;
use miette::IntoDiagnostic;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::constant::DEFAULT_CHANNEL;
use crate::toolchain::ordering::{
    NumericVersion, read_channel_ranks, read_release_positions, release_order,
};

pub mod atomic;
pub mod index;
pub mod ordering;
pub mod package;
pub mod resolve;
pub mod version;

/// The filename of the install metadata stub, relative to an install directory.
///
/// New installs write JSON (`{"channel": ..., "key": ...}`); legacy installs
/// wrote a plain-text release key (floating installs only). Both forms are
/// read; legacy files are left as-is until the toolchain is reinstalled or
/// updated, which rewrites the stub in the JSON format.
pub(crate) const METADATA_FILENAME: &str = "version";

/// The recorded identity of an installed toolchain.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct InstallMeta {
    /// The channel the toolchain was installed from
    pub channel: String,

    /// The installed release key: the build date for dated releases,
    /// otherwise the version
    pub key: String,
}

/// Parse a metadata stub's content. Legacy plain-text stubs hold only the
/// release key (and only existed on floating installs), so the channel is
/// supplied by the caller.
fn parse_meta(content: &str, legacy_channel: String) -> Option<InstallMeta> {
    if let Ok(meta) = serde_json::from_str::<InstallMeta>(content) {
        return Some(meta);
    }
    let key = content.trim();
    (!key.is_empty()).then_some(InstallMeta {
        channel: legacy_channel,
        key: key.to_owned(),
    })
}

impl From<InstallMeta> for ToolchainSpec {
    fn from(meta: InstallMeta) -> Self {
        ToolchainSpec::Version {
            channel: meta.channel,
            key: meta.key,
        }
    }
}

/// A toolchain spec contains a channel name that does not match Moonup's
/// channel-name grammar.
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

/// Install specification for a toolchain
///
/// A floating spec names a valid channel advertised by the dist server's main
/// index. Channel names use lowercase ASCII letters, digits, and hyphens.
/// Pinned versions are `(channel, key)` pairs; the key uniquely identifies the
/// release within its channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolchainSpec {
    /// A floating channel spec: the newest release of the named channel
    Channel(String),

    /// A pinned release within a channel.
    ///
    /// Input syntax is `{channel}<sep>{key}` with `<sep>` `/` or `@`. The
    /// legacy `nightly-YYYY-MM-DD` form is also accepted.
    Version {
        /// The channel the pin resolves within
        channel: String,
        /// The release key
        key: String,
    },
}

impl ToolchainSpec {
    /// The dist server channel this spec resolves within.
    #[inline]
    pub fn channel(&self) -> &str {
        match self {
            ToolchainSpec::Channel(c) => c,
            ToolchainSpec::Version { channel, .. } => channel,
        }
    }

    /// The canonical string form of this spec: bare key for default-channel
    /// pins, `{channel}@{key}` otherwise. This is the form written to pin
    /// files and propagated to shims.
    pub fn canonical(&self) -> String {
        match self {
            ToolchainSpec::Channel(c) => c.clone(),
            ToolchainSpec::Version { channel, key } if channel == DEFAULT_CHANNEL => key.clone(),
            ToolchainSpec::Version { channel, key } => format!("{channel}@{key}"),
        }
    }

    /// Validate the channel carried by this spec.
    fn validate_channel_name(&self) -> Result<(), InvalidChannelName> {
        let channel = self.channel();
        if crate::dist_server::schema::is_valid_channel_name(channel) {
            Ok(())
        } else {
            Err(InvalidChannelName {
                channel: channel.to_owned(),
            })
        }
    }

    /// Parse a toolchain spec from its syntactic string form.
    fn parse_syntactically(value: &str) -> Self {
        // Qualified pin input uses `/` or `@`. A hyphen is part of a channel
        // name except for the exact legacy nightly-date form below.
        if let Some((channel, key)) = for_separators(value) {
            return ToolchainSpec::Version { channel, key };
        }

        if let Some(key) = legacy_nightly_key(value) {
            return ToolchainSpec::Version {
                channel: "nightly".to_owned(),
                key: key.to_owned(),
            };
        }

        // Bare, version-shaped string: a pin in the default channel.
        if value.contains('+') || version::is_stable_selector(value) {
            return ToolchainSpec::Version {
                channel: DEFAULT_CHANNEL.to_owned(),
                key: value.to_owned(),
            };
        }

        ToolchainSpec::Channel(value.to_owned())
    }

    /// The toolchains directory basename for this spec: the channel name for
    /// floating installs, the bare key for default-channel pins, and
    /// `{channel}-{key}` for other channels' pins.
    pub fn install_name(&self) -> String {
        match self {
            ToolchainSpec::Channel(c) => c.clone(),
            ToolchainSpec::Version { channel, key } if channel == DEFAULT_CHANNEL => key.clone(),
            ToolchainSpec::Version { channel, key } => format!("{channel}-{key}"),
        }
    }

    //// Get the install dir root for the toolchain
    pub fn install_path(&self) -> PathBuf {
        let mut path = crate::moonup_home().join("toolchains");
        path.push(self.install_name());
        path
    }

    /// Get the install metadata for this toolchain, if available.
    ///
    /// Returns `None` if the metadata file is not present or cannot be parsed.
    pub fn install_meta(&self) -> Option<InstallMeta> {
        let meta_path = self.install_path().join(METADATA_FILENAME);
        if meta_path.is_file() {
            let content = std::fs::read_to_string(&meta_path).ok()?;
            parse_meta(&content, self.install_name())
        } else {
            None
        }
    }
}

impl std::fmt::Display for ToolchainSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.canonical())
    }
}

impl TryFrom<&str> for ToolchainSpec {
    type Error = InvalidChannelName;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let spec = Self::parse_syntactically(value);
        spec.validate_channel_name()?;
        Ok(spec)
    }
}

impl TryFrom<String> for ToolchainSpec {
    type Error = InvalidChannelName;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_from(value.as_str())
    }
}

/// A toolchain installation found in the local inventory.
#[derive(Debug, Clone)]
pub struct InstalledToolchain {
    /// The toolchain's floating-channel or pinned-version identity.
    pub spec: ToolchainSpec,

    /// The current release key for a floating installation, if known.
    /// Pinned specs already contain their release key.
    pub release_key: Option<String>,
}

impl InstalledToolchain {
    pub fn from_path(path: &Path) -> miette::Result<Self> {
        let name = path
            .file_name()
            .map(|n| n.to_ascii_lowercase().to_string_lossy().to_string())
            .ok_or_else(|| miette::miette!("failed to read toolchain install name"))?;

        let meta_path = path.join(METADATA_FILENAME);
        if meta_path.is_file() {
            let content = std::fs::read_to_string(&meta_path)
                .into_diagnostic()
                .inspect_err(|e| tracing::warn!("failed to read toolchain metadata stub {}", e))?;

            if let Some(meta) = parse_meta(&content, name.clone()) {
                // Floating installs live at the channel's own name. Legacy
                // plain-text stubs are left unchanged on this read path.
                let floating = name == meta.channel;
                let (spec, release_key) = if floating {
                    (ToolchainSpec::Channel(meta.channel), Some(meta.key))
                } else {
                    (
                        ToolchainSpec::Version {
                            channel: meta.channel,
                            key: meta.key,
                        },
                        None,
                    )
                };

                return Ok(Self { spec, release_key });
            }
        }

        // Metadata-free directories share the spec parser's legacy naming
        // rules: dated nightly pins and bare latest keys are still recognized,
        // while other channel-name hyphens are not treated as separators.
        let spec = ToolchainSpec::parse_syntactically(&name);

        Ok(Self {
            spec,
            release_key: None,
        })
    }
}

/// Split a string as a qualified pin `{channel}<sep>{key}` (`<sep>` is `/` or
/// `@`).
fn for_separators(s: &str) -> Option<(String, String)> {
    for sep in ['/', '@'] {
        if let Some((channel, key)) = s.split_once(sep)
            && !channel.is_empty()
            && !key.is_empty()
        {
            return Some((channel.to_owned(), key.to_owned()));
        }
    }
    None
}

/// Return the key from the legacy `nightly-YYYY-MM-DD` form when the date is
/// an actual calendar date.
fn legacy_nightly_key(s: &str) -> Option<&str> {
    let key = s.strip_prefix("nightly-")?;
    let mut parts = key.split('-');
    let (year, month, day) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some()
        || year.len() != 4
        || month.len() != 2
        || day.len() != 2
        || ![year, month, day]
            .iter()
            .all(|part| part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return None;
    }

    NaiveDate::from_ymd_opt(year.parse().ok()?, month.parse().ok()?, day.parse().ok()?)?;
    Some(key)
}

pub fn installed_toolchains() -> miette::Result<Vec<InstalledToolchain>> {
    let toolchains_dir = crate::moonup_home().join("toolchains");

    let toolchains = match toolchains_dir.read_dir() {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => vec![],
        Err(e) => return Err(miette::miette!(e).wrap_err("failed to read toolchains directory")),
        Ok(read_dir) => {
            let mut t = read_dir
                .filter_map(std::io::Result::ok)
                // skip internal directories such as `.staging`
                .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
                .filter_map(|e| InstalledToolchain::from_path(&e.path()).ok())
                .collect::<Vec<_>>();

            let mut pinned_keys: HashMap<String, Vec<String>> = HashMap::new();
            for it in &t {
                if let ToolchainSpec::Version { channel, key } = &it.spec
                    && NumericVersion::parse(key).is_ok()
                {
                    pinned_keys
                        .entry(channel.clone())
                        .or_default()
                        .push(key.clone());
                }
            }
            let release_positions = pinned_keys
                .into_iter()
                .map(|(channel, keys)| {
                    let positions = read_release_positions(&channel, &keys);
                    (channel, positions)
                })
                .collect();
            let channel_ranks = read_channel_ranks();
            t.sort_by_key(|it| release_order(&it.spec, &release_positions, &channel_ranks));
            t
        }
    };

    Ok(toolchains)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(value: &str) -> ToolchainSpec {
        ToolchainSpec::try_from(value).expect("test spec should be valid")
    }

    #[test]
    fn parses_qualified_pins_with_slash_and_at() {
        // Both accepted separators parse to the same (channel, key) pair.
        let expected = ToolchainSpec::Version {
            channel: "lts".to_owned(),
            key: "0.11.0".to_owned(),
        };
        assert_eq!(spec("lts/0.11.0"), expected);
        assert_eq!(spec("lts@0.11.0"), expected);
        assert!(ToolchainSpec::try_from("lts-0.11.0").is_err());
    }

    #[test]
    fn parses_legacy_nightly_pins_and_dates() {
        assert_eq!(
            spec("nightly-2026-01-01"),
            ToolchainSpec::Version {
                channel: "nightly".to_owned(),
                key: "2026-01-01".to_owned(),
            }
        );
        assert_eq!(
            spec("nightly-2026-02-31"),
            ToolchainSpec::Channel("nightly-2026-02-31".to_owned())
        );
        assert_eq!(
            spec("nightly-2026-1-01"),
            ToolchainSpec::Channel("nightly-2026-1-01".to_owned())
        );
    }

    #[test]
    fn validates_channel_names_when_parsing_user_specs() {
        assert_eq!(
            ToolchainSpec::try_from("dev-preview").expect("hyphenated channel is valid"),
            ToolchainSpec::Channel("dev-preview".to_owned())
        );
        assert!(ToolchainSpec::try_from("Dev-preview").is_err());
        assert!(ToolchainSpec::try_from("dev_preview").is_err());
        assert_eq!(
            ToolchainSpec::try_from("nightly-2026-01-01").expect("legacy nightly date is accepted"),
            ToolchainSpec::Version {
                channel: "nightly".to_owned(),
                key: "2026-01-01".to_owned(),
            }
        );
    }

    #[test]
    fn parses_bare_versions_as_default_channel_pins() {
        assert_eq!(
            spec("0.11.0"),
            ToolchainSpec::Version {
                channel: DEFAULT_CHANNEL.to_owned(),
                key: "0.11.0".to_owned(),
            }
        );
        assert_eq!(
            spec("0.10"),
            ToolchainSpec::Version {
                channel: DEFAULT_CHANNEL.to_owned(),
                key: "0.10".to_owned(),
            }
        );
        assert_eq!(
            spec("0.11.0+abc"),
            ToolchainSpec::Version {
                channel: DEFAULT_CHANNEL.to_owned(),
                key: "0.11.0+abc".to_owned(),
            }
        );
    }

    #[test]
    fn parses_plain_names_as_floating_channels() {
        assert_eq!(spec("lts"), ToolchainSpec::Channel("lts".to_owned()));
        assert_eq!(
            spec("nightly"),
            ToolchainSpec::Channel("nightly".to_owned())
        );
    }

    #[test]
    fn canonicalizes_default_channel_pins_to_bare_keys() {
        assert_eq!(spec("latest/0.11.0").canonical(), "0.11.0");
        assert_eq!(spec("0.11.0").canonical(), "0.11.0");
        assert_eq!(spec("lts/0.11.0").canonical(), "lts@0.11.0");
        assert_eq!(spec("lts@0.11.0").canonical(), "lts@0.11.0");
        // Legacy dashed nightly pins canonicalize to the new form.
        assert_eq!(spec("nightly-2026-01-01").canonical(), "nightly@2026-01-01");
        assert_eq!(spec("nightly/2026-01-01").canonical(), "nightly@2026-01-01");
    }

    #[test]
    fn maps_specs_to_flat_install_names() {
        assert_eq!(spec("latest").install_name(), "latest");
        assert_eq!(spec("lts").install_name(), "lts");
        // default-channel pins keep the bare dir name
        assert_eq!(spec("0.11.0").install_name(), "0.11.0");
        // Other channels' pins keep their flat, dashed directory names.
        assert_eq!(spec("lts/0.11.0").install_name(), "lts-0.11.0");
        assert_eq!(
            spec("nightly-2026-01-01").install_name(),
            "nightly-2026-01-01"
        );
    }

    #[test]
    fn qualified_pins_from_different_channels_do_not_collide() {
        let latest = spec("0.11.0");
        let lts = spec("lts/0.11.0");

        assert_ne!(latest, lts);
        assert_ne!(latest.install_path(), lts.install_path());
        assert_eq!(latest.channel(), DEFAULT_CHANNEL);
        assert_eq!(lts.channel(), "lts");
    }

    #[test]
    fn classifies_install_dirs_from_metadata() {
        let tempdir = assert_fs::TempDir::new().expect("should create tempdir");

        // floating channel install: dir name == channel name
        let floating = tempdir.path().join("lts");
        std::fs::create_dir_all(&floating).expect("should create dir");
        std::fs::write(
            floating.join(METADATA_FILENAME),
            r#"{"channel":"lts","key":"1.0.0"}"#,
        )
        .expect("should write stub");
        let parsed = InstalledToolchain::from_path(&floating).expect("should classify");
        assert_eq!(parsed.spec, ToolchainSpec::Channel("lts".to_owned()));
        assert_eq!(parsed.release_key.as_deref(), Some("1.0.0"));

        // qualified pinned install: dir name != channel name
        let pinned = tempdir.path().join("lts-1.0.0");
        std::fs::create_dir_all(&pinned).expect("should create dir");
        std::fs::write(
            pinned.join(METADATA_FILENAME),
            r#"{"channel":"lts","key":"1.0.0"}"#,
        )
        .expect("should write stub");
        let parsed = InstalledToolchain::from_path(&pinned).expect("should classify");
        assert_eq!(
            parsed.spec,
            ToolchainSpec::Version {
                channel: "lts".to_owned(),
                key: "1.0.0".to_owned(),
            }
        );
        assert_eq!(parsed.release_key, None);

        // Metadata keeps hyphens in both the channel name and release key
        // unambiguous for new installs.
        let prerelease = tempdir.path().join("dev-preview-1.0.0-preview");
        std::fs::create_dir_all(&prerelease).expect("should create dir");
        std::fs::write(
            prerelease.join(METADATA_FILENAME),
            r#"{"channel":"dev-preview","key":"1.0.0-preview"}"#,
        )
        .expect("should write stub");
        let parsed = InstalledToolchain::from_path(&prerelease).expect("should classify");
        assert_eq!(
            parsed.spec,
            ToolchainSpec::Version {
                channel: "dev-preview".to_owned(),
                key: "1.0.0-preview".to_owned(),
            }
        );
        assert_eq!(parsed.release_key, None);

        // legacy plain-text stub: floating, left as-is on read paths
        let legacy = tempdir.path().join("nightly");
        std::fs::create_dir_all(&legacy).expect("should create dir");
        std::fs::write(legacy.join(METADATA_FILENAME), "2026-01-01\n")
            .expect("should write legacy stub");
        let parsed = InstalledToolchain::from_path(&legacy).expect("should classify");
        assert_eq!(parsed.spec, ToolchainSpec::Channel("nightly".to_owned()));
        assert_eq!(parsed.release_key.as_deref(), Some("2026-01-01"));
        // read paths must not mutate the install; the stub upgrades to JSON
        // only when the toolchain is reinstalled or updated
        let stub =
            std::fs::read_to_string(legacy.join(METADATA_FILENAME)).expect("should read stub");
        assert_eq!(stub, "2026-01-01\n");

        // Legacy latest pins with build metadata remain identifiable by their
        // bare version directory names.
        let latest_pin = tempdir.path().join("0.10.17+d971670cd");
        std::fs::create_dir_all(&latest_pin).expect("should create dir");
        let parsed = InstalledToolchain::from_path(&latest_pin).expect("should classify");
        assert_eq!(
            parsed.spec,
            ToolchainSpec::Version {
                channel: DEFAULT_CHANNEL.to_owned(),
                key: "0.10.17+d971670cd".to_owned(),
            }
        );

        // legacy pinned install: no stub at all
        let legacy_pinned = tempdir.path().join("nightly-2025-06-01");
        std::fs::create_dir_all(&legacy_pinned).expect("should create dir");
        let parsed = InstalledToolchain::from_path(&legacy_pinned).expect("should classify");
        assert_eq!(
            parsed.spec,
            ToolchainSpec::Version {
                channel: "nightly".to_owned(),
                key: "2025-06-01".to_owned(),
            }
        );

        // A hyphenated channel directory is not inferred as a qualified pin.
        let channel = tempdir.path().join("dev-preview");
        std::fs::create_dir_all(&channel).expect("should create dir");
        let parsed = InstalledToolchain::from_path(&channel).expect("should classify");
        assert_eq!(
            parsed.spec,
            ToolchainSpec::Channel("dev-preview".to_owned())
        );
    }
}

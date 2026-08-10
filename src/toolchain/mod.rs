use miette::IntoDiagnostic;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::dist_server::schema::{ChannelIndex, ChannelName};

pub mod atomic;
pub mod index;
pub mod package;
pub mod resolve;

/// Install specification for a toolchain
///
/// This can be a specific version, or one of the special values:
/// - `latest`: the latest stable release
/// - `nightly`: the latest nightly build
/// - `bleeding`: the latest build from the main branch
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolchainSpec {
    /// `latest` toolchain
    Latest,

    /// `nightly` toolchain
    Nightly,

    /// `bleeding` toolchain
    Bleeding,

    /// A specific version of the toolchain
    /// This can be a version number (e.g., "1.0.0") or a nightly
    /// build (e.g., "nightly-2025-01-01")
    Version(String),
}

impl ToolchainSpec {
    /// Check if the spec is set to 'latest'
    #[inline]
    pub fn is_latest(&self) -> bool {
        matches!(self, ToolchainSpec::Latest)
    }

    /// Check if the spec is set to 'nightly'
    #[inline]
    pub fn is_nightly(&self) -> bool {
        matches!(self, ToolchainSpec::Nightly)
    }

    /// Check if the spec is set to 'bleeding'
    #[inline]
    pub fn is_bleeding(&self) -> bool {
        matches!(self, ToolchainSpec::Bleeding)
    }

    //// Get the install dir root for the toolchain
    pub fn install_path(&self) -> PathBuf {
        let mut path = crate::moonup_home().join("toolchains");
        path.push(self.to_string());
        path
    }

    pub fn as_str(&self) -> &str {
        match self {
            ToolchainSpec::Latest => "latest",
            ToolchainSpec::Nightly => "nightly",
            ToolchainSpec::Bleeding => "bleeding",
            ToolchainSpec::Version(v) => v.as_str(),
        }
    }
}

impl std::fmt::Display for ToolchainSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolchainSpec::Latest => write!(f, "latest"),
            ToolchainSpec::Nightly => write!(f, "nightly"),
            ToolchainSpec::Bleeding => write!(f, "bleeding"),
            ToolchainSpec::Version(v) => write!(f, "{}", v),
        }
    }
}

impl From<&str> for ToolchainSpec {
    fn from(s: &str) -> Self {
        ToolchainSpec::from(s.to_string())
    }
}

impl From<String> for ToolchainSpec {
    fn from(s: String) -> Self {
        match s.as_str() {
            "latest" => ToolchainSpec::Latest,
            "nightly" => ToolchainSpec::Nightly,
            "bleeding" => ToolchainSpec::Bleeding,
            _ => ToolchainSpec::Version(s),
        }
    }
}

/// Derives a `ChannelName` from a `ToolchainSpec`.
impl From<&ToolchainSpec> for ChannelName {
    fn from(spec: &ToolchainSpec) -> Self {
        match spec {
            ToolchainSpec::Latest => ChannelName::Latest,
            ToolchainSpec::Nightly => ChannelName::Nightly,
            ToolchainSpec::Bleeding => ChannelName::Bleeding,
            ToolchainSpec::Version(v) => {
                if v.starts_with("nightly") {
                    ChannelName::Nightly
                } else {
                    ChannelName::Latest
                }
            }
        }
    }
}

/// Installed toolchain information
#[derive(Debug, Clone)]
pub struct InstalledToolchain {
    /// The install name of the installed toolchain
    pub name: ToolchainSpec,

    /// The actual version tag (compiler version / build date) of the
    /// installed toolchain
    pub tag: Option<String>,
}

impl InstalledToolchain {
    pub fn from_path(path: &Path) -> miette::Result<Self> {
        let n = path
            .file_name()
            .map(|n| n.to_ascii_lowercase().to_string_lossy().to_string())
            .ok_or_else(|| miette::miette!("failed to read toolchain install name"))?;

        let name = ToolchainSpec::from(n);
        Ok(name.into())
    }
}

impl From<ToolchainSpec> for InstalledToolchain {
    fn from(spec: ToolchainSpec) -> Self {
        let tag = match &spec {
            ToolchainSpec::Version(_) => None,
            _ => Some(
                std::fs::read_to_string(spec.install_path().join("version"))
                    .map(|s| s.trim().to_owned())
                    .into_diagnostic()
                    .inspect_err(|e| tracing::warn!("failed to read toolchain version stub {}", e))
                    .unwrap_or("unknown".to_owned()),
            ),
        };

        Self { name: spec, tag }
    }
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

            // List in release order: versioned installs grouped by channel
            // (nightly builds by build date, latest releases by their position
            // in the cached `latest` channel index), followed by the channel
            // aliases `bleeding` < `nightly` < `latest`.
            let latest_positions = read_latest_channel_positions(&t);
            t.sort_by(|a, b| {
                release_order(&a.name, &latest_positions)
                    .cmp(&release_order(&b.name, &latest_positions))
            });
            t
        }
    };

    Ok(toolchains)
}

/// The release order of an installed toolchain, oldest first.
///
/// Versioned installs precede the floating channel aliases, grouped by channel
/// and ordered by release date. The aliases sort `bleeding` < `nightly` <
/// `latest`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum ReleaseOrder {
    /// A versioned nightly install, ordered by its build date (YYYY-MM-DD)
    Nightly(String),
    /// A versioned latest install, ordered by its position in the cached
    /// `latest` channel index (a release absent from the index is older and
    /// sorts first), then numerically by version
    Latest(Option<usize>, NumericVersion),
    /// A floating channel alias: `bleeding` < `nightly` < `latest`
    Channel(u8),
}

/// A version number compared component-wise, so `0.10.6` orders after `0.9.0`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct NumericVersion(Vec<u64>);

impl NumericVersion {
    fn parse(version: &str) -> Self {
        // ignore the `+build` metadata suffix (e.g. `0.10.6+80dc50f24`)
        let core = version.split('+').next().unwrap_or(version);
        let parts = core
            .split('.')
            .map(|p| p.parse::<u64>().unwrap_or(0))
            .collect();
        NumericVersion(parts)
    }
}

/// Derive the [`ReleaseOrder`] sort key for an installed toolchain.
fn release_order(spec: &ToolchainSpec, latest_positions: &HashMap<String, usize>) -> ReleaseOrder {
    match spec {
        ToolchainSpec::Bleeding => ReleaseOrder::Channel(0),
        ToolchainSpec::Nightly => ReleaseOrder::Channel(1),
        ToolchainSpec::Latest => ReleaseOrder::Channel(2),
        ToolchainSpec::Version(v) if v.starts_with("nightly") => {
            ReleaseOrder::Nightly(v.trim_start_matches("nightly-").to_owned())
        }
        ToolchainSpec::Version(v) => {
            ReleaseOrder::Latest(latest_positions.get(v).copied(), NumericVersion::parse(v))
        }
    }
}

/// Positions of latest releases in the cached `latest` channel index, used to
/// order latest-channel installs. Offline and best-effort: on any read or
/// parse error the installs fall back to numeric version ordering.
fn read_latest_channel_positions(installs: &[InstalledToolchain]) -> HashMap<String, usize> {
    let latest_versions = installs
        .iter()
        .filter_map(|t| match &t.name {
            ToolchainSpec::Version(v) if !v.starts_with("nightly") => Some(v.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    if latest_versions.is_empty() {
        return HashMap::new();
    }

    let path = crate::moonup_home()
        .join("downloads")
        .join("channel-latest.json");
    let Ok(content) = std::fs::read_to_string(&path) else {
        return HashMap::new();
    };
    let Ok(index) = serde_json::from_str::<ChannelIndex>(&content) else {
        return HashMap::new();
    };

    let positions = index
        .releases()
        .iter()
        .enumerate()
        .map(|(i, r)| (r.version.clone(), i))
        .collect::<HashMap<_, _>>();

    // A stale cache that lacks one of the installed releases would otherwise
    // sort that release (as `None`) below older releases that are present in
    // the index, breaking oldest-to-newest order. If the cache is incomplete,
    // fall back to pure numeric version ordering for the whole group.
    if latest_versions.iter().any(|v| !positions.contains_key(v)) {
        return HashMap::new();
    }

    positions
}

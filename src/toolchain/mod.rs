use miette::IntoDiagnostic;
use std::path::{Path, PathBuf};

use crate::dist_server::schema::ChannelName;

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
    Latest,
    Nightly,
    Bleeding,
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

impl Ord for ToolchainSpec {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            // latest
            (ToolchainSpec::Latest, ToolchainSpec::Latest) => std::cmp::Ordering::Equal,
            (ToolchainSpec::Latest, ToolchainSpec::Nightly) => std::cmp::Ordering::Less,
            (ToolchainSpec::Latest, ToolchainSpec::Bleeding) => std::cmp::Ordering::Less,
            (ToolchainSpec::Latest, ToolchainSpec::Version(s)) => {
                if s.starts_with("nightly") {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Greater
                }
            }
            // nightly
            (ToolchainSpec::Nightly, ToolchainSpec::Nightly) => std::cmp::Ordering::Equal,
            (ToolchainSpec::Nightly, ToolchainSpec::Latest) => std::cmp::Ordering::Greater,
            (ToolchainSpec::Nightly, ToolchainSpec::Bleeding) => std::cmp::Ordering::Less,
            (ToolchainSpec::Nightly, ToolchainSpec::Version(_)) => std::cmp::Ordering::Greater,
            // bleeding
            (ToolchainSpec::Bleeding, ToolchainSpec::Bleeding) => std::cmp::Ordering::Equal,
            (ToolchainSpec::Bleeding, ToolchainSpec::Latest) => std::cmp::Ordering::Greater,
            (ToolchainSpec::Bleeding, ToolchainSpec::Nightly) => std::cmp::Ordering::Greater,
            (ToolchainSpec::Bleeding, ToolchainSpec::Version(_)) => std::cmp::Ordering::Greater,
            // version
            (ToolchainSpec::Version(a), ToolchainSpec::Version(b)) => a.cmp(b),
            (ToolchainSpec::Version(s), ToolchainSpec::Latest) => {
                if s.starts_with("nightly") {
                    std::cmp::Ordering::Greater
                } else {
                    std::cmp::Ordering::Less
                }
            }
            (ToolchainSpec::Version(_), ToolchainSpec::Nightly) => std::cmp::Ordering::Less,
            (ToolchainSpec::Version(_), ToolchainSpec::Bleeding) => std::cmp::Ordering::Less,
        }
    }
}

impl PartialOrd for ToolchainSpec {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
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
        let tag = match &name {
            ToolchainSpec::Version(_) => None,
            _ => Some(
                std::fs::read_to_string(path.join("version"))
                    .map(|s| s.trim().to_owned())
                    .into_diagnostic()
                    .inspect_err(|e| tracing::warn!("failed to read toolchain version stub {}", e))
                    .unwrap_or("unknown".to_owned()),
            ),
        };

        Ok(Self { name, tag })
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
                .filter_map(|e| InstalledToolchain::from_path(&e.path()).ok())
                .collect::<Vec<_>>();
            t.sort_by_key(|t| t.name.clone());
            t
        }
    };

    Ok(toolchains)
}

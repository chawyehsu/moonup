use miette::IntoDiagnostic;
use std::path::{Path, PathBuf};

use crate::toolchain::ordering::{
    read_channel_ranks, read_latest_channel_positions, release_order,
};

pub mod atomic;
pub mod index;
pub mod ordering;
pub mod package;
pub mod resolve;
pub mod version;

/// Install specification for a toolchain
///
/// Any channel name advertised by the dist server's main index is a valid spec
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolchainSpec {
    /// A floating channel spec: the newest release of the named channel
    Channel(String),

    /// A concrete toolchain version
    ///
    /// This can be a version number (e.g., "0.10.1+abc") or a dated build
    /// (e.g., "nightly-2026-01-01")
    Version(String),
}

impl ToolchainSpec {
    /// The install name of the toolchain, also its toolchains directory name
    #[inline]
    pub fn as_str(&self) -> &str {
        match self {
            ToolchainSpec::Channel(c) => c,
            ToolchainSpec::Version(v) => v,
        }
    }

    /// The dist server channel this spec resolves within.
    ///
    /// A floating channel spec resolves within its own channel. Pinned dated
    /// builds (`nightly-YYYY-MM-DD`) belong to the `nightly` channel; every
    /// other pinned version searches the `latest` channel.
    #[inline]
    pub fn channel(&self) -> &str {
        match self {
            ToolchainSpec::Channel(c) => c,
            ToolchainSpec::Version(v) if v.starts_with("nightly-") => "nightly",
            ToolchainSpec::Version(_) => "latest",
        }
    }

    //// Get the install dir root for the toolchain
    pub fn install_path(&self) -> PathBuf {
        let mut path = crate::moonup_home().join("toolchains");
        path.push(self.as_str());
        path
    }
}

impl std::fmt::Display for ToolchainSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl From<&str> for ToolchainSpec {
    fn from(s: &str) -> Self {
        ToolchainSpec::from(s.to_string())
    }
}

impl From<String> for ToolchainSpec {
    fn from(s: String) -> Self {
        // Syntactic classification, refined against the main index at resolve
        // time (`toolchain::index::interpret_spec`): pinned dated builds and
        // version-shaped strings are versions; any other string is a channel
        // name.
        if s.starts_with("nightly-") || s.contains('+') || version::is_stable_selector(&s) {
            ToolchainSpec::Version(s)
        } else {
            ToolchainSpec::Channel(s)
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

        // Floating channel installs carry a `version` stub recording the
        // installed release; pinned installs do not.
        let spec = if path.join("version").is_file() {
            ToolchainSpec::Channel(n)
        } else {
            ToolchainSpec::Version(n)
        };
        Ok(spec.into())
    }
}

impl From<ToolchainSpec> for InstalledToolchain {
    fn from(spec: ToolchainSpec) -> Self {
        let tag = match &spec {
            ToolchainSpec::Version(_) => None,
            ToolchainSpec::Channel(_) => Some(
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

            // List in release order: pinned installs precede floating channel
            // installs, grouped by channel (dated builds by build date, latest
            // releases by their position in the cached `latest` channel index);
            // floating channels follow their order in the cached main index.
            let latest_positions = read_latest_channel_positions(&t);
            let channel_ranks = read_channel_ranks();
            t.sort_by_key(|it| release_order(&it.name, &latest_positions, &channel_ranks));
            t
        }
    };

    Ok(toolchains)
}

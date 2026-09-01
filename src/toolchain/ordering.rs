//! Ordering for concrete releases in oldest-to-newest channel sequences.
use std::cmp::Ordering;
use std::collections::HashMap;

use crate::{
    dist_server::schema::ChannelIndex,
    toolchain::{InstalledToolchain, ToolchainSpec},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompareError {
    InvalidVersion(String),
    ReleaseNotFound(String),
    EmptySequence,
}

/// A version number compared component-wise, so `0.10.6` orders after `0.9.0`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct NumericVersion(Vec<u64>);

impl NumericVersion {
    pub fn parse(value: &str) -> Result<Self, CompareError> {
        let (core, build) = value
            .split_once('+')
            .map_or((value, None), |(c, b)| (c, Some(b)));
        if core.is_empty() || build.is_some_and(str::is_empty) {
            return Err(CompareError::InvalidVersion(value.into()));
        }
        let parts = core.split('.').collect::<Vec<_>>();
        if parts.is_empty()
            || parts.len() > 3
            || parts
                .iter()
                .any(|p| p.is_empty() || !p.chars().all(|c| c.is_ascii_digit()))
            || build.is_some_and(|b| b.chars().any(|c| c.is_whitespace() || c == '+'))
        {
            return Err(CompareError::InvalidVersion(value.into()));
        }
        parts
            .into_iter()
            .map(|p| {
                p.parse::<u64>()
                    .map_err(|_| CompareError::InvalidVersion(value.into()))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(NumericVersion)
    }
}

/// Compare two releases in a channel sequence, returning their relative order.
///
/// # Returns
///
/// The ordering of `a` relative to `b` in the channel sequence.
pub fn compare<'a>(
    releases: impl IntoIterator<Item = &'a str>,
    a: &str,
    b: &str,
) -> Result<Ordering, CompareError> {
    let releases = releases.into_iter().collect::<Vec<_>>();
    if releases.is_empty() {
        return Err(CompareError::EmptySequence);
    }
    let a_position = releases
        .iter()
        .position(|release| *release == a)
        .ok_or_else(|| CompareError::ReleaseNotFound(a.into()))?;
    let b_position = releases
        .iter()
        .position(|release| *release == b)
        .ok_or_else(|| CompareError::ReleaseNotFound(b.into()))?;
    Ok(a_position.cmp(&b_position))
}

/// The release order of an installed toolchain, oldest first.
///
/// Versioned installs precede the floating channel aliases, grouped by channel
/// and ordered by release date. The aliases sort `bleeding` < `nightly` <
/// `latest`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReleaseOrder {
    /// A versioned nightly install, ordered by its build date (YYYY-MM-DD)
    Nightly(String),
    /// A versioned latest install, ordered by its position in the cached
    /// `latest` channel index. A release absent from the index triggers
    /// a fallback to numeric version ordering for the whole group
    Latest(Option<usize>, NumericVersion),
    /// A floating channel alias: `bleeding` < `nightly` < `latest`
    Channel(u8),
}

/// Derive the [`ReleaseOrder`] sort key for an installed toolchain.
pub fn release_order(
    spec: &ToolchainSpec,
    latest_positions: &HashMap<String, usize>,
) -> ReleaseOrder {
    match spec {
        ToolchainSpec::Bleeding => ReleaseOrder::Channel(0),
        ToolchainSpec::Nightly => ReleaseOrder::Channel(1),
        ToolchainSpec::Latest => ReleaseOrder::Channel(2),
        ToolchainSpec::Version(v) if v.starts_with("nightly") => {
            ReleaseOrder::Nightly(v.trim_start_matches("nightly-").to_owned())
        }
        ToolchainSpec::Version(v) => ReleaseOrder::Latest(
            latest_positions.get(v).copied(),
            NumericVersion::parse(v).unwrap_or_default(),
        ),
    }
}

/// Positions of latest releases in the cached `latest` channel index, used to
/// order latest-channel installs. Offline and best-effort: on any read or
/// parse error the installs fall back to numeric version ordering.
pub fn read_latest_channel_positions(installs: &[InstalledToolchain]) -> HashMap<String, usize> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_exact_identities() {
        let r = ["0.10.6+old", "0.10.6+new", "0.10.7"];
        assert_eq!(
            compare(r, "0.10.6+new", "0.10.6+old"),
            Ok(Ordering::Greater)
        );
        assert_eq!(compare(r, "0.10.6+old", "0.10.7"), Ok(Ordering::Less));
        assert_eq!(compare(r, "0.10.6+new", "0.10.6+new"), Ok(Ordering::Equal));
    }

    #[test]
    fn reports_missing_releases_and_empty_sequence() {
        assert!(matches!(
            compare(["0.1"], "0.2", "0.1"),
            Err(CompareError::ReleaseNotFound(_))
        ));
        assert_eq!(compare([], "0.1", "0.2"), Err(CompareError::EmptySequence));
    }
}

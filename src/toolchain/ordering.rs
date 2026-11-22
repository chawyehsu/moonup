//! Ordering for concrete releases in oldest-to-newest channel sequences.
use std::cmp::Ordering;
use std::collections::HashMap;

use crate::{
    dist_server::schema::{ChannelIndex, Index},
    toolchain::ToolchainSpec,
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
/// Pinned installs precede floating channel installs, grouped by channel
/// and ordered by release date / index position. Floating channels follow
/// their order in the main index; channels absent from the index sort
/// last, by name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReleaseOrder {
    /// A pinned dated build, ordered by its build date (YYYY-MM-DD)
    Dated(String),
    /// A pinned versioned build, ordered by its position in the channel
    /// index. A release absent from the index triggers a fallback to
    /// numeric version ordering for the whole group
    Versioned(Option<usize>, NumericVersion),
    /// A floating channel install, ordered by its rank in the main
    /// index, then by name
    Channel(usize, String),
}

/// Derive the [`ReleaseOrder`] sort key for an installed toolchain.
pub fn release_order(
    spec: &ToolchainSpec,
    release_positions: &HashMap<String, HashMap<String, usize>>,
    channel_ranks: &HashMap<String, usize>,
) -> ReleaseOrder {
    match spec {
        ToolchainSpec::Channel(c) => ReleaseOrder::Channel(
            channel_ranks.get(c).copied().unwrap_or(usize::MAX),
            c.clone(),
        ),
        ToolchainSpec::Version { channel, key } => match NumericVersion::parse(key) {
            Ok(num) => ReleaseOrder::Versioned(
                release_positions
                    .get(channel)
                    .and_then(|positions| positions.get(key))
                    .copied(),
                num,
            ),
            // Non-numeric keys are dated builds (YYYY-MM-DD); order them by
            // their date string
            Err(_) => ReleaseOrder::Dated(key.clone()),
        },
    }
}

/// Positions of releases in a channel's cached channel index.
///
/// Offline and best-effort: on any read or parse error the installs
/// fall back to numeric version ordering.
pub fn read_release_positions(
    channel: &str,
    keys: &[String],
    channel_ranks: &HashMap<String, usize>,
) -> HashMap<String, usize> {
    if keys.is_empty() {
        return HashMap::new();
    }

    let path = crate::moonup_home()
        .join("downloads")
        .join(format!("channel-{channel}.json"));
    let Ok(content) = std::fs::read_to_string(&path) else {
        return HashMap::new();
    };
    let Ok(index) = serde_json::from_str::<ChannelIndex>(&content) else {
        return HashMap::new();
    };

    // add the channel rank to the release positions so all releases from the
    // same channel are grouped together according to the channel rank.
    let channel_rank = channel_ranks.get(channel).copied().unwrap_or(0);
    let positions = index
        .releases()
        .iter()
        .enumerate()
        .map(|(i, r)| (r.key().to_owned(), i + channel_rank))
        .collect::<HashMap<_, _>>();

    // A stale cache that lacks one of the installed releases would otherwise
    // sort that release (as `None`) below older releases that are present in
    // the index, breaking oldest-to-newest order. If the cache is incomplete,
    // fall back to pure numeric version ordering for the whole group.
    if keys.iter().any(|key| !positions.contains_key(key)) {
        return HashMap::new();
    }

    positions
}

/// Ranks of channels in the cached main index, used to order floating
/// channel installs.
///
/// Offline and best-effort: on any read or parse error no channel is
/// ranked, and unranked channels sort last, by name.
pub fn read_channel_ranks() -> HashMap<String, usize> {
    let path = crate::moonup_home().join("downloads").join("index.json");
    let Ok(content) = std::fs::read_to_string(&path) else {
        return HashMap::new();
    };
    let Ok(index) = serde_json::from_str::<Index>(&content) else {
        return HashMap::new();
    };

    index
        .channels()
        .iter()
        .enumerate()
        .map(|(i, c)| (c.name.clone(), i))
        .collect()
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

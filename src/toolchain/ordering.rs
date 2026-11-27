//! Ordering for concrete releases in oldest-to-newest channel sequences.
use std::cmp::Ordering;
use std::collections::HashMap;

use crate::{
    dist_server::schema::{ChannelIndex, Index},
    session::Session,
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
    /// A pinned build, grouped by channel before comparing its release order.
    Pinned {
        /// Rank in the main index; unranked channels sort last.
        channel_rank: usize,
        /// Name breaks ties between unranked channels.
        channel: String,
        /// Release ordering within this channel.
        order: PinnedReleaseOrder,
    },
    /// A floating channel install, ordered by its rank in the main
    /// index, then by name
    Channel(usize, String),
}

/// Ordering of a pinned release within its channel.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PinnedReleaseOrder {
    /// A dated build, ordered by its build date (YYYY-MM-DD).
    Dated(String),
    /// A versioned build, ordered by its position in the channel index.
    /// A release absent from the index triggers numeric version ordering.
    Versioned(Option<usize>, NumericVersion),
}

/// Derive the [`ReleaseOrder`] sort key for an installed toolchain.
pub fn release_order(
    spec: &ToolchainSpec,
    release_positions: &HashMap<String, HashMap<String, usize>>,
    channel_ranks: &HashMap<String, usize>,
) -> ReleaseOrder {
    match spec {
        ToolchainSpec::Channel(c) => ReleaseOrder::Channel(
            channel_ranks.get(c.as_str()).copied().unwrap_or(usize::MAX),
            c.to_string(),
        ),
        ToolchainSpec::Version { channel, key } => ReleaseOrder::Pinned {
            channel_rank: channel_ranks
                .get(channel.as_str())
                .copied()
                .unwrap_or(usize::MAX),
            channel: channel.to_string(),
            order: match NumericVersion::parse(key) {
                Ok(num) => PinnedReleaseOrder::Versioned(
                    release_positions
                        .get(channel.as_str())
                        .and_then(|positions| positions.get(key))
                        .copied(),
                    num,
                ),
                // Non-numeric keys are dated builds (YYYY-MM-DD); order them
                // by their date string within the channel.
                Err(_) => PinnedReleaseOrder::Dated(key.clone()),
            },
        },
    }
}

/// Positions of releases in a channel's channel index.
///
/// Offline and best-effort: reads the session in offline mode (memory or
/// disk, never the network); on any error the installs fall back to numeric
/// version ordering.
pub fn read_release_positions(
    session: &Session,
    channel: &str,
    keys: &[String],
) -> HashMap<String, usize> {
    if keys.is_empty() {
        return HashMap::new();
    }

    session
        .read_channel_index_offline(channel)
        .map(|index| release_positions(&index, keys))
        .unwrap_or_default()
}

fn release_positions(index: &ChannelIndex, keys: &[String]) -> HashMap<String, usize> {
    let positions = index
        .releases()
        .iter()
        .enumerate()
        .map(|(i, r)| (r.key().to_owned(), i))
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

/// Ranks of channels in the main index, used to order floating
/// channel installs.
///
/// Offline and best-effort: reads the session in offline mode (memory or
/// disk, never the network); on any error no channel is ranked, and
/// unranked channels sort last, by name.
pub fn read_channel_ranks(session: &Session) -> HashMap<String, usize> {
    session
        .read_index_offline()
        .map(|index| channel_ranks(&index))
        .unwrap_or_default()
}

fn channel_ranks(index: &Index) -> HashMap<String, usize> {
    index
        .channels()
        .iter()
        .enumerate()
        .map(|(i, c)| (c.name.to_string(), i))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dist_server::schema::ChannelName;

    fn channel_name(value: &str) -> ChannelName {
        ChannelName::parse(value).expect("test channel name should be valid")
    }

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

    #[test]
    fn groups_pinned_installs_by_channel_before_release_order() {
        let channel_ranks = HashMap::from([("first".to_owned(), 0), ("second".to_owned(), 1)]);
        let release_positions = HashMap::from([(
            "first".to_owned(),
            HashMap::from([("0.1.0".to_owned(), 10)]),
        )]);
        let first = ToolchainSpec::Version {
            channel: channel_name("first"),
            key: "0.1.0".to_owned(),
        };
        let second = ToolchainSpec::Version {
            channel: channel_name("second"),
            key: "99.0.0".to_owned(),
        };

        assert!(
            release_order(&first, &release_positions, &channel_ranks)
                < release_order(&second, &release_positions, &channel_ranks),
            "channel rank must dominate release position and numeric version"
        );

        // Pinned release kinds are also compared only after the channel, so a
        // ranked versioned channel precedes a later-ranked dated channel.
        let dated = ToolchainSpec::Version {
            channel: channel_name("second"),
            key: "2026-01-01".to_owned(),
        };
        assert!(
            release_order(&first, &release_positions, &channel_ranks)
                < release_order(&dated, &release_positions, &channel_ranks)
        );
    }

    #[test]
    fn orders_pinned_release_positions_within_a_channel() {
        let channel_ranks = HashMap::from([("latest".to_owned(), 0)]);
        let release_positions = HashMap::from([(
            "latest".to_owned(),
            HashMap::from([("0.9.0".to_owned(), 1), ("0.10.0".to_owned(), 0)]),
        )]);
        let older_index_position = ToolchainSpec::Version {
            channel: channel_name("latest"),
            key: "0.10.0".to_owned(),
        };
        let newer_index_position = ToolchainSpec::Version {
            channel: channel_name("latest"),
            key: "0.9.0".to_owned(),
        };

        assert!(
            release_order(&older_index_position, &release_positions, &channel_ranks)
                < release_order(&newer_index_position, &release_positions, &channel_ranks)
        );
    }
}

use crate::dist_server::schema::Release;

/// Whether a version is a numeric stable selector rather than a concrete
/// build-qualified version.
pub fn is_stable_selector(value: &str) -> bool {
    !value.contains('+') && {
        let parts = value.split('.').collect::<Vec<_>>();
        (1..=3).contains(&parts.len())
            && parts
                .iter()
                .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
    }
}

/// Resolve a numeric stable-version selector against an oldest-first release list.
pub fn resolve_stable_selector<'a>(
    selector: &str,
    mut releases: impl Iterator<Item = &'a Release>,
) -> Option<&'a Release> {
    if selector.contains('+') {
        return releases.find(|release| release.version == selector);
    }
    if !is_stable_selector(selector) {
        return None;
    }
    let parts = selector.split('.').collect::<Vec<_>>();

    let matching = releases
        .filter(|release| {
            release
                .version
                .split('+')
                .next()
                .map(|base| {
                    base.split('.').take(parts.len()).eq(parts.iter().copied())
                        && base.split('.').count() >= parts.len()
                })
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();

    if parts.len() == 3 {
        matching
            .iter()
            .copied()
            .find(|release| release.version == selector)
            .or_else(|| {
                matching
                    .into_iter()
                    .rev()
                    .find(|release| release.version.contains('+'))
            })
    } else {
        matching.into_iter().next_back()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn releases(values: &[&str]) -> Vec<Release> {
        values
            .iter()
            .map(|version| Release::new(*version))
            .collect()
    }

    #[test]
    fn resolves_prefixes_and_three_part_precedence() {
        let releases = releases(&["0.9.0", "0.10.0+old", "0.10.1+build", "0.10.1"]);
        assert_eq!(
            resolve_stable_selector("0", releases.iter())
                .unwrap()
                .version,
            "0.10.1"
        );
        assert_eq!(
            resolve_stable_selector("0.10", releases.iter())
                .unwrap()
                .version,
            "0.10.1"
        );
        assert_eq!(
            resolve_stable_selector("0.10.1", releases.iter())
                .unwrap()
                .version,
            "0.10.1"
        );
    }

    #[test]
    fn falls_back_to_last_build_and_rejects_malformed_selectors() {
        let releases = releases(&["0.10.1+old", "0.10.1+new"]);
        assert_eq!(
            resolve_stable_selector("0.10.1", releases.iter())
                .unwrap()
                .version,
            "0.10.1+new"
        );
        assert!(resolve_stable_selector("v0.10", releases.iter()).is_none());
        assert!(resolve_stable_selector("0.10.1.2", releases.iter()).is_none());
    }
}

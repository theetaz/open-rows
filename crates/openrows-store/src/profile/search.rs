// SPDX-License-Identifier: Apache-2.0
//! Sidebar search and grouping over loaded profiles (FR-CONN-01).

use std::collections::BTreeSet;

use super::ConnectionProfile;

/// Case-insensitive match of every whitespace-separated term against the
/// profile's name, group, engine, host or file, database, user or
/// environment label.
#[must_use]
pub fn matches(profile: &ConnectionProfile, query: &str) -> bool {
    let file = profile
        .file
        .as_ref()
        .map(|file| file.path.to_string_lossy().into_owned());
    let haystack = [
        Some(profile.name.as_str()),
        profile.group.as_deref(),
        Some(profile.driver.name()),
        file.as_deref(),
        (!profile.driver.is_file()).then_some(profile.params.host.as_str()),
        profile.params.database.as_deref(),
        profile.params.user.as_deref(),
        profile.env_tag.label.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join("\n")
    .to_lowercase();
    query
        .split_whitespace()
        .all(|term| haystack.contains(&term.to_lowercase()))
}

/// Distinct, sorted group names in use.
#[must_use]
pub fn groups(profiles: &[ConnectionProfile]) -> Vec<String> {
    profiles
        .iter()
        .filter_map(|profile| profile.group.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

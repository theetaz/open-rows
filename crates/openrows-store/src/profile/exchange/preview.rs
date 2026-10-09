// SPDX-License-Identifier: Apache-2.0
//! Conflict classification and validated confirmation decisions.
use super::{ExchangeError, ImportBatch, equivalent};
use crate::profile::{ConnectionProfile, ProfileId, ProfileService};

/// One incoming row's conflict with currently saved profiles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Conflict {
    /// No matching name or equivalent profile.
    New,
    /// A profile with this name already exists.
    SameName(ProfileId),
    /// All portable fields already exist; skip automatically.
    Identical,
}
/// User-selected action for one preview row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportChoice {
    /// Leave existing profiles untouched.
    Skip,
    /// Replace the same-name entry under a new id.
    Replace,
    /// Create a new profile, adding a unique suffix when needed.
    KeepBoth,
}
/// Confirmed action and explicit opt-in for an incoming password.
#[derive(Clone, Copy, Debug)]
pub struct ImportDecision {
    /// Conflict action.
    pub choice: ImportChoice,
    /// Save the separately held password in the OS keychain.
    pub save_password: bool,
}
/// Classify the complete preview without touching files or the keychain.
/// # Errors
/// A profile cannot be serialized for comparison.
pub fn preview(
    batch: &ImportBatch,
    existing: &[ConnectionProfile],
) -> Result<Vec<Conflict>, ExchangeError> {
    batch
        .entries
        .iter()
        .map(|entry| classify(&entry.profile, existing))
        .collect()
}
fn classify(
    incoming: &ConnectionProfile,
    existing: &[ConnectionProfile],
) -> Result<Conflict, ExchangeError> {
    for profile in existing {
        if equivalent(incoming, profile)? {
            return Ok(Conflict::Identical);
        }
    }
    Ok(existing
        .iter()
        .find(|profile| profile.name == incoming.name)
        .map_or(Conflict::New, |profile| Conflict::SameName(profile.id)))
}
/// Apply a confirmed preview; validate every decision before any writes.
/// # Errors
/// Invalid decisions, profile validation or storage/keychain failures.
pub fn apply_import(
    service: &ProfileService,
    batch: ImportBatch,
    decisions: &[ImportDecision],
) -> Result<Vec<ConnectionProfile>, ExchangeError> {
    if decisions.len() != batch.entries.len() {
        return Err(ExchangeError::Invalid(
            "one decision per profile is required".into(),
        ));
    }
    let mut existing = service.list()?.profiles;
    let mut planned = Vec::new();
    for (entry, decision) in batch.entries.into_iter().zip(decisions) {
        let conflict = classify(&entry.profile, &existing)?;
        if decision.choice == ImportChoice::Skip || conflict == Conflict::Identical {
            continue;
        }
        let replaced = match (decision.choice, conflict) {
            (ImportChoice::Replace, Conflict::SameName(id)) => Some(id),
            (ImportChoice::Replace, Conflict::New) => {
                return Err(ExchangeError::Invalid(
                    "Replace requires a same-name profile; refresh the preview".into(),
                ));
            }
            _ => None,
        };
        let mut profile = entry.profile;
        profile.id = ProfileId::new();
        profile.imported = true;
        if replaced.is_none() {
            profile.name = unique_name(&profile.name, &existing);
        }
        profile.validate()?;
        existing.retain(|profile| Some(profile.id) != replaced);
        existing.push(profile.clone());
        planned.push((
            profile,
            entry.password.filter(|_| decision.save_password),
            replaced,
        ));
    }
    let mut saved = Vec::new();
    for (profile, password, replaced) in planned {
        let created = service.create(profile, password.as_ref())?;
        if let Some(id) = replaced {
            service.delete(id)?;
        }
        saved.push(created);
    }
    Ok(saved)
}
fn unique_name(name: &str, existing: &[ConnectionProfile]) -> String {
    if !existing.iter().any(|profile| profile.name == name) {
        return name.to_owned();
    }
    let mut suffix = 2;
    loop {
        let candidate = format!("{name} ({suffix})");
        if !existing.iter().any(|profile| profile.name == candidate) {
            return candidate;
        }
        suffix += 1;
    }
}

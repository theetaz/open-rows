// SPDX-License-Identifier: Apache-2.0
//! Profile CRUD that keeps passwords and passphrases in the secret store
//! (docs/12 §3).
//!
//! Order of operations: secrets are written before the profile file that
//! references them, and the file is removed before its secrets, so a crash
//! never leaves a profile pointing at a missing secret it claims to have.

use std::sync::Arc;

use openrows_secrets::{SecretStore, SecretString};

use super::{
    ConnectionProfile, LoadedProfiles, ProfileError, ProfileFiles, ProfileId, SecretChanges,
    SecretRef, SecretRefs, SecretSlot, groups, matches,
};

/// What an update does to a stored secret.
#[derive(Debug, Clone, Default)]
pub enum PasswordChange {
    /// Leave the stored secret as it is.
    #[default]
    Keep,
    /// Replace it with a new value.
    Set(SecretString),
    /// Remove it.
    Clear,
}

/// Create, edit, duplicate, delete, list, group and search profiles.
#[derive(Clone)]
pub struct ProfileService {
    files: ProfileFiles,
    secrets: Arc<dyn SecretStore>,
}

impl ProfileService {
    /// Profiles in `files`, secrets in `secrets`.
    #[must_use]
    pub fn new(files: ProfileFiles, secrets: Arc<dyn SecretStore>) -> Self {
        Self { files, secrets }
    }

    /// Save a new profile and, if given, its database password.
    ///
    /// # Errors
    /// `Invalid`, `AlreadyExists`, `Secret`, `Io` or `Json`.
    pub fn create(
        &self,
        profile: ConnectionProfile,
        password: Option<&SecretString>,
    ) -> Result<ConnectionProfile, ProfileError> {
        let change = password.map_or(PasswordChange::Keep, |value| {
            PasswordChange::Set(value.clone())
        });
        self.create_with(profile, &SecretChanges::password(change))
    }

    /// Save a new profile with every secret `changes` sets; secrets the
    /// profile's settings do not use are ignored.
    ///
    /// # Errors
    /// `Invalid`, `AlreadyExists`, `Secret`, `Io` or `Json`.
    pub fn create_with(
        &self,
        mut profile: ConnectionProfile,
        changes: &SecretChanges,
    ) -> Result<ConnectionProfile, ProfileError> {
        profile.validate()?;
        if self.files.exists(profile.id) {
            return Err(ProfileError::AlreadyExists {
                id: profile.id.to_string(),
            });
        }
        profile.secrets = SecretRefs::default();
        for slot in SecretSlot::ALL {
            if let PasswordChange::Set(value) = changes.get(slot)
                && profile.uses_secret(slot)
            {
                self.secrets.set(&profile.secret_key(slot), value)?;
                profile.secrets.set(slot, Some(SecretRef::Keychain));
            }
        }
        self.files.save(&profile)?;
        Ok(profile)
    }

    /// Replace an existing profile, applying a database password change.
    ///
    /// # Errors
    /// `Invalid`, `NotFound`, `Secret`, `Io` or `Json`.
    pub fn update(
        &self,
        profile: ConnectionProfile,
        password: PasswordChange,
    ) -> Result<ConnectionProfile, ProfileError> {
        self.update_with(profile, &SecretChanges::password(password))
    }

    /// Replace an existing profile, applying `changes`. Secrets the new
    /// settings no longer use (SSH turned off, no client key) are removed.
    ///
    /// # Errors
    /// `Invalid`, `NotFound`, `Secret`, `Io` or `Json`.
    pub fn update_with(
        &self,
        mut profile: ConnectionProfile,
        changes: &SecretChanges,
    ) -> Result<ConnectionProfile, ProfileError> {
        profile.validate()?;
        let had = self.files.load(profile.id)?.secrets;
        for slot in SecretSlot::ALL {
            let stored = match changes.get(slot) {
                _ if !profile.uses_secret(slot) => None,
                PasswordChange::Keep => had.get(slot),
                PasswordChange::Set(value) => {
                    self.secrets.set(&profile.secret_key(slot), value)?;
                    Some(SecretRef::Keychain)
                }
                PasswordChange::Clear => None,
            };
            profile.secrets.set(slot, stored);
        }
        self.files.save(&profile)?;
        // The store is only touched for secrets that exist: every keychain
        // access by an untrusted binary is a macOS prompt.
        for slot in SecretSlot::ALL {
            if had.get(slot).is_some() && profile.secrets.get(slot).is_none() {
                self.secrets.delete(&profile.secret_key(slot))?;
            }
        }
        Ok(profile)
    }

    /// Copy profile `id` (and its secrets) under a new id named "<name> copy".
    ///
    /// # Errors
    /// `NotFound`, `Secret`, `Io` or `Json`.
    pub fn duplicate(&self, id: ProfileId) -> Result<ConnectionProfile, ProfileError> {
        let source = self.files.load(id)?;
        let mut changes = SecretChanges::default();
        for slot in SecretSlot::ALL {
            if let Some(value) = self.secret(&source, slot)? {
                *changes.get_mut(slot) = PasswordChange::Set(value);
            }
        }
        let mut copy = source;
        copy.id = ProfileId::new();
        copy.name = format!("{} copy", copy.name);
        self.create_with(copy, &changes)
    }

    /// Delete profile `id` and its stored secrets.
    ///
    /// # Errors
    /// `NotFound`, `Secret` or `Io`.
    pub fn delete(&self, id: ProfileId) -> Result<(), ProfileError> {
        let profile = self.files.load(id)?;
        self.files.delete(id)?;
        for slot in SecretSlot::ALL {
            if profile.secrets.get(slot).is_some() {
                self.secrets.delete(&profile.secret_key(slot))?;
            }
        }
        Ok(())
    }

    /// Load profile `id`.
    ///
    /// # Errors
    /// `NotFound`, `Io`, `Json` or `UnsupportedFormat`.
    pub fn get(&self, id: ProfileId) -> Result<ConnectionProfile, ProfileError> {
        self.files.load(id)
    }

    /// All profiles, sorted by group then name.
    ///
    /// # Errors
    /// `Io` when the directory cannot be listed.
    pub fn list(&self) -> Result<LoadedProfiles, ProfileError> {
        self.files.load_all()
    }

    /// Profiles matching `query` (see [`matches`]).
    ///
    /// # Errors
    /// `Io` when the directory cannot be listed.
    pub fn search(&self, query: &str) -> Result<Vec<ConnectionProfile>, ProfileError> {
        let mut loaded = self.list()?;
        loaded.profiles.retain(|profile| matches(profile, query));
        Ok(loaded.profiles)
    }

    /// Distinct group names in use.
    ///
    /// # Errors
    /// `Io` when the directory cannot be listed.
    pub fn groups(&self) -> Result<Vec<String>, ProfileError> {
        Ok(groups(&self.list()?.profiles))
    }

    /// The stored database password of `profile`, for the connect path only.
    ///
    /// # Errors
    /// `Secret` when the store cannot be read.
    pub fn password(
        &self,
        profile: &ConnectionProfile,
    ) -> Result<Option<SecretString>, ProfileError> {
        self.secret(profile, SecretSlot::Password)
    }

    /// The stored secret in `slot`, for the connect path only. The webview
    /// never receives it (docs/12 §3). A slot without a reference never
    /// touches the secret store.
    ///
    /// # Errors
    /// `Secret` when the store cannot be read.
    pub fn secret(
        &self,
        profile: &ConnectionProfile,
        slot: SecretSlot,
    ) -> Result<Option<SecretString>, ProfileError> {
        if !profile.uses_secret(slot) {
            return Ok(None);
        }
        match profile.secrets.get(slot) {
            Some(SecretRef::Keychain) => Ok(self.secrets.get(&profile.secret_key(slot))?),
            None => Ok(None),
        }
    }
}

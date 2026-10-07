// SPDX-License-Identifier: Apache-2.0
//! Profile CRUD that keeps passwords in the secret store (docs/12 §3).
//!
//! Order of operations: the secret is written before the profile file that
//! references it, and the file is removed before its secret, so a crash never
//! leaves a profile pointing at a missing secret it claims to have.

use std::sync::Arc;

use openrows_secrets::{SecretStore, SecretString};

use super::{
    ConnectionProfile, LoadedProfiles, ProfileError, ProfileFiles, ProfileId, SecretRef, groups,
    matches,
};

/// What an update does to the stored password.
#[derive(Debug, Clone, Default)]
pub enum PasswordChange {
    /// Leave the stored password as it is.
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
    /// Profiles in `files`, passwords in `secrets`.
    #[must_use]
    pub fn new(files: ProfileFiles, secrets: Arc<dyn SecretStore>) -> Self {
        Self { files, secrets }
    }

    /// Save a new profile and, if given, its password.
    ///
    /// # Errors
    /// `Invalid`, `AlreadyExists`, `Secret`, `Io` or `Json`.
    pub fn create(
        &self,
        mut profile: ConnectionProfile,
        password: Option<&SecretString>,
    ) -> Result<ConnectionProfile, ProfileError> {
        profile.validate()?;
        if self.files.exists(profile.id) {
            return Err(ProfileError::AlreadyExists {
                id: profile.id.to_string(),
            });
        }
        profile.secrets.password = None;
        if let Some(password) = password {
            self.secrets.set(&profile.password_key(), password)?;
            profile.secrets.password = Some(SecretRef::Keychain);
        }
        self.files.save(&profile)?;
        Ok(profile)
    }

    /// Replace an existing profile, applying `password`.
    ///
    /// # Errors
    /// `Invalid`, `NotFound`, `Secret`, `Io` or `Json`.
    pub fn update(
        &self,
        mut profile: ConnectionProfile,
        password: PasswordChange,
    ) -> Result<ConnectionProfile, ProfileError> {
        profile.validate()?;
        let stored = self.files.load(profile.id)?;
        let key = profile.password_key();
        match password {
            PasswordChange::Keep => profile.secrets.password = stored.secrets.password,
            PasswordChange::Set(value) => {
                self.secrets.set(&key, &value)?;
                profile.secrets.password = Some(SecretRef::Keychain);
            }
            PasswordChange::Clear => profile.secrets.password = None,
        }
        self.files.save(&profile)?;
        if profile.secrets.password.is_none() {
            self.secrets.delete(&key)?;
        }
        Ok(profile)
    }

    /// Copy profile `id` (and its password) under a new id named "<name> copy".
    ///
    /// # Errors
    /// `NotFound`, `Secret`, `Io` or `Json`.
    pub fn duplicate(&self, id: ProfileId) -> Result<ConnectionProfile, ProfileError> {
        let source = self.files.load(id)?;
        let password = self.password(&source)?;
        let mut copy = source;
        copy.id = ProfileId::new();
        copy.name = format!("{} copy", copy.name);
        self.create(copy, password.as_ref())
    }

    /// Delete profile `id` and its stored password.
    ///
    /// # Errors
    /// `NotFound`, `Secret` or `Io`.
    pub fn delete(&self, id: ProfileId) -> Result<(), ProfileError> {
        let profile = self.files.load(id)?;
        self.files.delete(id)?;
        self.secrets.delete(&profile.password_key())?;
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

    /// The stored password of `profile`, for the connect path only. The
    /// webview never receives it (docs/12 §3).
    ///
    /// # Errors
    /// `Secret` when the store cannot be read.
    pub fn password(
        &self,
        profile: &ConnectionProfile,
    ) -> Result<Option<SecretString>, ProfileError> {
        match profile.secrets.password {
            Some(SecretRef::Keychain) => Ok(self.secrets.get(&profile.password_key())?),
            None => Ok(None),
        }
    }
}

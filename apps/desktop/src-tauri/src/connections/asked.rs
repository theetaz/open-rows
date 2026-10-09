// SPDX-License-Identifier: Apache-2.0
//! "Ask every time" database passwords (docs/12 §3): typed in the connect
//! prompt, held in memory (zeroised on removal) while the connection is
//! open, and never written to the keychain, a profile file or a log.
use openrows_secrets::{MemorySecretStore, SecretKey, SecretStore, SecretString};
use openrows_store::profile::{ConnectionProfile, ProfileError, ProfileId};

use super::error::{IpcError, IpcErrorKind};

/// Passwords of open "ask every time" connections, keyed by profile.
#[derive(Default)]
pub(crate) struct AskedPasswords(MemorySecretStore);

impl AskedPasswords {
    /// Hold `password` for profile `id` until [`Self::forget`].
    ///
    /// # Errors
    /// `Secret` when the in-memory store is poisoned.
    pub(crate) fn remember(&self, id: ProfileId, password: &SecretString) -> Result<(), IpcError> {
        self.0
            .set(&SecretKey::password(&id.to_string()), password)
            .map_err(secret_error)
    }

    /// The password typed for `profile` on connect.
    ///
    /// # Errors
    /// `Auth` when none was typed (the connection was closed since).
    pub(crate) fn password(&self, profile: &ConnectionProfile) -> Result<SecretString, IpcError> {
        self.0
            .get(&profile.password_key())
            .map_err(secret_error)?
            .ok_or_else(|| {
                IpcError::new(
                    IpcErrorKind::Auth,
                    "This connection asks for its password: connect again to enter it",
                )
            })
    }

    /// Drop the password of profile `id`; nothing happens when none is held.
    pub(crate) fn forget(&self, id: ProfileId) {
        // Removing from a poisoned map is impossible; the value then dies with the process.
        let _ = self.0.delete(&SecretKey::password(&id.to_string()));
    }
}

fn secret_error(error: openrows_secrets::SecretError) -> IpcError {
    ProfileError::from(error).into()
}

#[cfg(test)]
mod tests {
    use openrows_store::profile::{EnvKind, EnvTag, ServerParams};

    use super::*;

    #[test]
    fn passwords_live_until_forgotten() -> Result<(), IpcError> {
        let asked = AskedPasswords::default();
        let profile = ConnectionProfile::new_postgres(
            "p",
            ServerParams::default(),
            EnvTag::new(EnvKind::Dev),
        );
        let id = profile.id;
        assert!(
            asked
                .password(&profile)
                .is_err_and(|error| error.kind == IpcErrorKind::Auth)
        );
        asked.remember(id, &SecretString::from("pw-synthetic"))?;
        assert_eq!(asked.password(&profile)?.expose(), "pw-synthetic");
        asked.forget(id);
        asked.forget(id);
        assert!(asked.password(&profile).is_err());
        Ok(())
    }
}

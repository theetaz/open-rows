// SPDX-License-Identifier: Apache-2.0
//! Keychain addressing: one service, one account per stored secret.

use std::fmt;

/// Keychain service name shared by every `OpenRows` secret (docs/12 §3).
pub const SERVICE: &str = "com.prabhavalabs.openrows";

/// The keychain account a secret is stored under, e.g. `conn/<id>/password`.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct SecretKey {
    account: String,
}

impl SecretKey {
    /// Account for the database password of connection profile `profile_id`.
    #[must_use]
    pub fn password(profile_id: &str) -> Self {
        Self::for_profile(profile_id, "password")
    }

    /// Account for secret `item` (e.g. `ssh-passphrase`) of connection
    /// profile `profile_id`: `conn/<id>/<item>`.
    #[must_use]
    pub fn for_profile(profile_id: &str, item: &str) -> Self {
        Self {
            account: format!("conn/{profile_id}/{item}"),
        }
    }

    /// Account for an AI provider's API key; callers validate the identifier.
    #[must_use]
    pub fn ai_provider(provider_id: &str) -> Self {
        Self {
            account: format!("ai/{provider_id}/api-key"),
        }
    }

    /// The keychain account string.
    #[must_use]
    pub fn account(&self) -> &str {
        &self.account
    }
}

impl fmt::Debug for SecretKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "SecretKey({})", self.account)
    }
}

#[cfg(test)]
mod tests {
    use super::SecretKey;

    #[test]
    fn password_account_matches_docs() {
        assert_eq!(SecretKey::password("abc").account(), "conn/abc/password");
        assert_eq!(
            SecretKey::for_profile("abc", "ssh-passphrase").account(),
            "conn/abc/ssh-passphrase"
        );
    }
}

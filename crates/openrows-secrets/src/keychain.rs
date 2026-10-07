// SPDX-License-Identifier: Apache-2.0
//! OS credential store backend: macOS Keychain, Windows Credential Manager,
//! Linux kernel keyutils (all through the `keyring` crate).

use keyring::Entry;

use crate::{SERVICE, SecretError, SecretKey, SecretStore, SecretString};

/// Secrets stored in the platform credential store under [`SERVICE`].
#[derive(Debug, Default, Clone, Copy)]
pub struct KeychainSecretStore;

impl KeychainSecretStore {
    /// A handle to the platform credential store.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

fn entry(key: &SecretKey) -> Result<Entry, SecretError> {
    Entry::new(SERVICE, key.account()).map_err(|source| platform(key, source))
}

fn platform(key: &SecretKey, source: keyring::Error) -> SecretError {
    SecretError::Platform {
        account: key.account().to_owned(),
        source,
    }
}

impl SecretStore for KeychainSecretStore {
    fn set(&self, key: &SecretKey, value: &SecretString) -> Result<(), SecretError> {
        entry(key)?
            .set_password(value.expose())
            .map_err(|source| platform(key, source))
    }

    fn get(&self, key: &SecretKey) -> Result<Option<SecretString>, SecretError> {
        match entry(key)?.get_password() {
            Ok(value) => Ok(Some(SecretString::new(value))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(source) => Err(platform(key, source)),
        }
    }

    fn delete(&self, key: &SecretKey) -> Result<(), SecretError> {
        match entry(key)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(source) => Err(platform(key, source)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::KeychainSecretStore;
    use crate::{SecretError, SecretKey, SecretStore, SecretString};

    fn round_trip(
        store: &dyn SecretStore,
        key: &SecretKey,
    ) -> Result<(Option<SecretString>, Option<SecretString>), SecretError> {
        store.set(key, &SecretString::from("synthetic-test-value"))?;
        let read = store.get(key)?;
        store.delete(key)?;
        Ok((read, store.get(key)?))
    }

    /// Touches the real OS keychain with a throwaway item it always deletes.
    /// Run manually: `cargo test -p openrows-secrets -- --ignored`.
    #[test]
    #[ignore = "needs an unlocked OS keychain; run manually"]
    fn keychain_round_trip_with_throwaway_item() {
        let store = KeychainSecretStore::new();
        let id = format!("openrows-test-throwaway-{}", uuid::Uuid::now_v7());
        let key = SecretKey::password(&id);
        let outcome = round_trip(&store, &key);
        let cleanup = store.delete(&key);
        let (read, after_delete) = outcome.unwrap();
        cleanup.unwrap();
        assert_eq!(read.unwrap().expose(), "synthetic-test-value");
        assert!(after_delete.is_none());
    }
}

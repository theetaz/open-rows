// SPDX-License-Identifier: Apache-2.0
//! Process-local secret store for tests and "ask every time" sessions.

use std::{
    collections::HashMap,
    sync::{Mutex, MutexGuard},
};

use crate::{SecretError, SecretKey, SecretStore, SecretString};

type Entries = HashMap<SecretKey, SecretString>;

/// Secrets held in memory only; values are zeroized when removed or dropped.
#[derive(Debug, Default)]
pub struct MemorySecretStore {
    entries: Mutex<Entries>,
}

impl MemorySecretStore {
    /// An empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> Result<MutexGuard<'_, Entries>, SecretError> {
        self.entries.lock().map_err(|_| SecretError::Poisoned)
    }
}

impl SecretStore for MemorySecretStore {
    fn set(&self, key: &SecretKey, value: &SecretString) -> Result<(), SecretError> {
        self.lock()?.insert(key.clone(), value.clone());
        Ok(())
    }

    fn get(&self, key: &SecretKey) -> Result<Option<SecretString>, SecretError> {
        Ok(self.lock()?.get(key).cloned())
    }

    fn delete(&self, key: &SecretKey) -> Result<(), SecretError> {
        self.lock()?.remove(key);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::MemorySecretStore;
    use crate::{SecretKey, SecretStore, SecretString};

    #[test]
    fn set_get_delete_round_trip() {
        let store = MemorySecretStore::new();
        let key = SecretKey::password("p1");
        assert!(store.get(&key).unwrap().is_none());
        store
            .set(&key, &SecretString::from("s3cret-synthetic"))
            .unwrap();
        assert_eq!(
            store.get(&key).unwrap().unwrap().expose(),
            "s3cret-synthetic"
        );
        store.delete(&key).unwrap();
        store.delete(&key).unwrap();
        assert!(store.get(&key).unwrap().is_none());
    }

    #[test]
    fn debug_never_shows_values() {
        let store = MemorySecretStore::new();
        let value = SecretString::from("leak-canary-synthetic");
        store.set(&SecretKey::password("p1"), &value).unwrap();
        assert!(!format!("{store:?}").contains("leak-canary"));
    }
}

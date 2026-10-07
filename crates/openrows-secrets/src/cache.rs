// SPDX-License-Identifier: Apache-2.0
//! Process-wide read cache in front of a slower or prompting store.
//!
//! On macOS every keychain read by a binary the item's ACL does not trust
//! shows a password prompt, so the app reads each secret at most once per run.

use std::{
    collections::HashMap,
    sync::{Mutex, MutexGuard},
};

use crate::{SecretError, SecretKey, SecretStore, SecretString};

/// Known values by key; `None` records a key known to have no secret.
type Known = HashMap<SecretKey, Option<SecretString>>;

/// Wraps `S`, reading each key from it at most once until it is set or deleted.
///
/// Values stay [`SecretString`]s (zeroized when evicted or dropped) in process
/// memory only. Failed reads are not cached, so a denied prompt can be retried.
/// The lock is held across a backend read, so concurrent readers of a cold key
/// wait for one read (one prompt) instead of each starting their own.
#[derive(Debug)]
pub struct CachingSecretStore<S> {
    inner: S,
    known: Mutex<Known>,
}

impl<S: SecretStore> CachingSecretStore<S> {
    /// A cache in front of `inner`, initially empty.
    #[must_use]
    pub fn new(inner: S) -> Self {
        Self {
            inner,
            known: Mutex::new(Known::new()),
        }
    }

    fn lock(&self) -> Result<MutexGuard<'_, Known>, SecretError> {
        self.known.lock().map_err(|_| SecretError::Poisoned)
    }

    /// Record the outcome of a write: the new state on success, unknown on failure.
    fn settle(
        known: &mut Known,
        key: &SecretKey,
        outcome: Result<(), SecretError>,
        value: Option<SecretString>,
    ) -> Result<(), SecretError> {
        match outcome {
            Ok(()) => {
                known.insert(key.clone(), value);
                Ok(())
            }
            Err(error) => {
                known.remove(key);
                Err(error)
            }
        }
    }
}

impl<S: SecretStore> SecretStore for CachingSecretStore<S> {
    fn set(&self, key: &SecretKey, value: &SecretString) -> Result<(), SecretError> {
        let mut known = self.lock()?;
        if known
            .get(key)
            .is_some_and(|cached| cached.as_ref() == Some(value))
        {
            return Ok(());
        }
        let outcome = self.inner.set(key, value);
        Self::settle(&mut known, key, outcome, Some(value.clone()))
    }

    fn get(&self, key: &SecretKey) -> Result<Option<SecretString>, SecretError> {
        let mut known = self.lock()?;
        if let Some(cached) = known.get(key) {
            return Ok(cached.clone());
        }
        let value = self.inner.get(key)?;
        known.insert(key.clone(), value.clone());
        Ok(value)
    }

    fn delete(&self, key: &SecretKey) -> Result<(), SecretError> {
        let mut known = self.lock()?;
        if known.get(key).is_some_and(Option::is_none) {
            return Ok(());
        }
        let outcome = self.inner.delete(key);
        Self::settle(&mut known, key, outcome, None)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::CachingSecretStore;
    use crate::{MemorySecretStore, SecretError, SecretKey, SecretStore, SecretString};

    /// Counts backend calls; optionally fails every call.
    #[derive(Default)]
    struct Counting {
        store: MemorySecretStore,
        gets: AtomicUsize,
        sets: AtomicUsize,
        deletes: AtomicUsize,
        failing: bool,
    }

    impl Counting {
        fn calls(&self) -> (usize, usize, usize) {
            let read = |count: &AtomicUsize| count.load(Ordering::SeqCst);
            (read(&self.gets), read(&self.sets), read(&self.deletes))
        }

        fn check(&self, count: &AtomicUsize) -> Result<(), SecretError> {
            count.fetch_add(1, Ordering::SeqCst);
            if self.failing {
                return Err(SecretError::Poisoned);
            }
            Ok(())
        }
    }

    impl SecretStore for Counting {
        fn set(&self, key: &SecretKey, value: &SecretString) -> Result<(), SecretError> {
            self.check(&self.sets)?;
            self.store.set(key, value)
        }

        fn get(&self, key: &SecretKey) -> Result<Option<SecretString>, SecretError> {
            self.check(&self.gets)?;
            self.store.get(key)
        }

        fn delete(&self, key: &SecretKey) -> Result<(), SecretError> {
            self.check(&self.deletes)?;
            self.store.delete(key)
        }
    }

    fn exposed(value: Option<SecretString>) -> Option<String> {
        value.map(|secret| secret.expose().to_owned())
    }

    #[test]
    fn reads_hit_the_backend_once() -> Result<(), SecretError> {
        let backend = Counting::default();
        let key = SecretKey::password("p1");
        backend
            .store
            .set(&key, &SecretString::from("pw-synthetic"))?;
        let cache = CachingSecretStore::new(backend);
        for _ in 0..5 {
            assert_eq!(exposed(cache.get(&key)?).as_deref(), Some("pw-synthetic"));
        }
        assert_eq!(cache.inner.calls(), (1, 0, 0));
        Ok(())
    }

    #[test]
    fn absent_values_are_cached_too() -> Result<(), SecretError> {
        let cache = CachingSecretStore::new(Counting::default());
        let key = SecretKey::password("missing");
        assert!(cache.get(&key)?.is_none());
        assert!(cache.get(&key)?.is_none());
        cache.delete(&key)?;
        assert_eq!(cache.inner.calls(), (1, 0, 0));
        Ok(())
    }

    #[test]
    fn set_and_delete_update_the_cache_without_rereading() -> Result<(), SecretError> {
        let cache = CachingSecretStore::new(Counting::default());
        let key = SecretKey::password("p1");
        cache.set(&key, &SecretString::from("first-synthetic"))?;
        assert_eq!(
            exposed(cache.get(&key)?).as_deref(),
            Some("first-synthetic")
        );
        cache.set(&key, &SecretString::from("first-synthetic"))?;
        cache.set(&key, &SecretString::from("second-synthetic"))?;
        assert_eq!(
            exposed(cache.get(&key)?).as_deref(),
            Some("second-synthetic")
        );
        cache.delete(&key)?;
        assert!(cache.get(&key)?.is_none());
        assert!(cache.inner.store.get(&key)?.is_none());
        assert_eq!(cache.inner.calls(), (0, 2, 1));
        Ok(())
    }

    #[test]
    fn failures_are_not_cached() {
        let backend = Counting {
            failing: true,
            ..Counting::default()
        };
        let cache = CachingSecretStore::new(backend);
        let key = SecretKey::password("p1");
        assert!(cache.get(&key).is_err());
        assert!(cache.get(&key).is_err());
        assert!(
            cache
                .set(&key, &SecretString::from("pw-synthetic"))
                .is_err()
        );
        assert!(cache.get(&key).is_err());
        assert_eq!(cache.inner.calls(), (3, 1, 0));
    }

    #[test]
    fn debug_never_shows_values() -> Result<(), SecretError> {
        let cache = CachingSecretStore::new(MemorySecretStore::new());
        cache.set(
            &SecretKey::password("p1"),
            &SecretString::from("leak-canary-synthetic"),
        )?;
        assert!(!format!("{cache:?}").contains("leak-canary"));
        Ok(())
    }
}

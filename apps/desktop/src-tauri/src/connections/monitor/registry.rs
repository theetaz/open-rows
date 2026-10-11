// SPDX-License-Identifier: Apache-2.0
//! Shared admission and generation invalidation, including in-flight opens.
use openrows_monitor::{Monitor, MonitorError};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex, PoisonError},
};
pub(super) type Key = (String, String);
pub(super) struct Slot {
    pub(super) cancel: tokio::sync::watch::Sender<bool>,
    pub(super) monitor: Option<Arc<Monitor>>,
}
/// App-wide collector registry; pending opens count against the four-session cap.
#[derive(Default)]
pub(crate) struct Monitors {
    slots: Mutex<BTreeMap<Key, Arc<Mutex<Slot>>>>,
    connections: Mutex<(BTreeSet<String>, BTreeMap<String, u64>)>,
}
impl Monitors {
    pub(super) fn contains(&self, key: &Key) -> bool {
        self.slots
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .contains_key(key)
    }
    /// Capture the profile generation before an asynchronous connect.
    pub(crate) fn connection_generation(&self, profile: &str) -> u64 {
        let profile = profile_key(profile);
        *self
            .connections
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .1
            .get(&profile)
            .unwrap_or(&0)
    }
    /// Admit only a connect not invalidated by close or profile changes.
    pub(crate) fn connected(&self, profile: &str, generation: u64) {
        let profile = profile_key(profile);
        let mut connections = self
            .connections
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if *connections.1.get(&profile).unwrap_or(&0) == generation {
            connections.0.insert(profile);
        }
    }
    pub(super) fn reserve(&self, key: &Key) -> Result<(Arc<Mutex<Slot>>, bool), MonitorError> {
        let connections = self
            .connections
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if !connections.0.contains(&key.0) {
            return Err(MonitorError::Stopped);
        }
        let mut slots = self.slots.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(slot) = slots.get(key) {
            return Ok((slot.clone(), false));
        }
        if slots.len() >= 4 {
            return Err(MonitorError::Limit);
        }
        let (cancel, _) = tokio::sync::watch::channel(false);
        let slot = Arc::new(Mutex::new(Slot {
            cancel,
            monitor: None,
        }));
        slots.insert(key.clone(), slot.clone());
        Ok((slot, true))
    }
    pub(super) fn get(&self, key: &Key) -> Result<Arc<Monitor>, MonitorError> {
        self.slots
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(key)
            .and_then(|slot| {
                slot.lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .monitor
                    .clone()
            })
            .ok_or(MonitorError::Stopped)
    }
    pub(super) fn stop(&self, key: &Key) {
        if let Some(slot) = self
            .slots
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(key)
        {
            stop(&slot);
        }
    }
    /// A cancelled open must never remove a replacement reservation.
    pub(super) fn stop_reserved(&self, key: &Key, reservation: &Arc<Mutex<Slot>>) {
        let mut slots = self.slots.lock().unwrap_or_else(PoisonError::into_inner);
        if slots
            .get(key)
            .is_some_and(|current| Arc::ptr_eq(current, reservation))
            && let Some(slot) = slots.remove(key)
        {
            stop(&slot);
        }
    }
    /// Cancel pending and active generations before disconnecting a profile.
    pub(crate) fn stop_profile(&self, profile: &str) {
        let profile = profile_key(profile);
        let mut connections = self
            .connections
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        connections.0.remove(&profile);
        *connections.1.entry(profile.clone()).or_default() += 1;
        self.stop_where(|key| key.0 == profile);
    }
    /// Cancel all collectors while preserving explicit connection admission.
    pub(crate) fn stop_all(&self) {
        self.stop_where(|_| true);
    }
    fn stop_where(&self, matches: impl Fn(&Key) -> bool) {
        let mut slots = self.slots.lock().unwrap_or_else(PoisonError::into_inner);
        let keys: Vec<_> = slots.keys().filter(|key| matches(key)).cloned().collect();
        for key in keys {
            if let Some(slot) = slots.remove(&key) {
                stop(&slot);
            }
        }
    }
}
/// Canonical UUID aliases share one collector and invalidation generation.
pub(super) fn profile_key(profile: &str) -> String {
    super::super::profiles::profile_id(profile)
        .map_or_else(|_| profile.to_owned(), |id| id.to_string())
}
fn stop(slot: &Mutex<Slot>) {
    let mut slot = slot.lock().unwrap_or_else(PoisonError::into_inner);
    slot.cancel.send_replace(true);
    if let Some(monitor) = slot.monitor.take() {
        monitor.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_admission_caps_and_close_lock_invalidate_pending_generations()
    -> Result<(), MonitorError> {
        let registry = Monitors::default();
        let key = ("p".into(), "db".into());
        registry.connected("p", 0);
        let (slot, fresh) = registry.reserve(&key)?;
        assert!(fresh);
        assert!(!registry.reserve(&key)?.1);
        for index in 0..3 {
            registry.connected(&index.to_string(), 0);
            registry.reserve(&(index.to_string(), "db".into()))?;
        }
        registry.connected("extra", 0);
        assert!(matches!(
            registry.reserve(&("extra".into(), "db".into())),
            Err(MonitorError::Limit)
        ));
        registry.stop_profile("p");
        assert!(
            *slot
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .cancel
                .borrow()
        );
        assert!(registry.reserve(&key).is_err());
        registry.connected("p", 0);
        assert!(registry.reserve(&key).is_err());
        registry.connected("p", 1);
        let (next, _) = registry.reserve(&key)?;
        registry.stop_all();
        assert!(
            *next
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .cancel
                .borrow()
        );
        assert!(registry.reserve(&key)?.1);
        let upper = "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA";
        let lower = profile_key(upper);
        registry.connected(upper, 0);
        let (alias, _) = registry.reserve(&(lower, "db".into()))?;
        registry.stop_profile(upper);
        assert!(
            *alias
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .cancel
                .borrow()
        );
        Ok(())
    }
    #[test]
    fn cancelled_open_cleanup_preserves_the_replacement() -> Result<(), MonitorError> {
        let registry = Monitors::default();
        let key = ("p".into(), "db".into());
        registry.connected("p", 0);
        let (previous, _) = registry.reserve(&key)?;
        registry.stop(&key);
        let (replacement, _) = registry.reserve(&key)?;
        registry.stop_reserved(&key, &previous);
        assert!(registry.contains(&key));
        assert!(
            !*replacement
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .cancel
                .borrow()
        );
        registry.stop_reserved(&key, &replacement);
        assert!(!registry.contains(&key));
        Ok(())
    }
}

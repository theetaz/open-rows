// SPDX-License-Identifier: Apache-2.0
//! Stored results an export is reading stay in the store until it ends,
//! even when their tab closes and releases them, so closing a tab never
//! breaks a running export.
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, MutexGuard, PoisonError};

#[derive(Default)]
struct Pins {
    /// Running exports per result id.
    readers: HashMap<String, usize>,
    /// Pinned results their tab released meanwhile.
    released: HashSet<String>,
}

/// Results read by running exports.
#[derive(Default)]
pub(crate) struct ResultPins(Mutex<Pins>);

impl ResultPins {
    fn lock(&self) -> MutexGuard<'_, Pins> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// An export starts reading `id`.
    pub(crate) fn pin(&self, id: &str) {
        *self.lock().readers.entry(id.to_owned()).or_default() += 1;
    }

    /// The tab released `id`: true when it can be removed now, false when
    /// the last export reading it removes it.
    pub(crate) fn release(&self, id: &str) -> bool {
        let mut pins = self.lock();
        if pins.readers.contains_key(id) {
            pins.released.insert(id.to_owned());
            return false;
        }
        true
    }

    /// An export stopped reading `id`: true when the tab released it
    /// meanwhile and it can be removed now.
    pub(crate) fn unpin(&self, id: &str) -> bool {
        let mut pins = self.lock();
        let Some(readers) = pins.readers.get_mut(id) else {
            return false;
        };
        *readers -= 1;
        if *readers > 0 {
            return false;
        }
        pins.readers.remove(id);
        pins.released.remove(id)
    }
}

#[cfg(test)]
mod tests {
    use super::ResultPins;

    #[test]
    fn a_released_result_is_removed_by_its_last_reader() {
        let pins = ResultPins::default();
        assert!(pins.release("free"));
        pins.pin("r");
        pins.pin("r");
        assert!(!pins.release("r"));
        assert!(!pins.unpin("r"));
        assert!(pins.unpin("r"));
        pins.pin("kept");
        assert!(!pins.unpin("kept"));
        assert!(pins.release("kept"));
    }
}

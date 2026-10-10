// SPDX-License-Identifier: Apache-2.0
//! Nonblocking hosted-stream admission, separate from legacy append semantics.
use crate::ResultStore;
use std::sync::atomic::Ordering;
/// Parent result-store admission for the next hosted batch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Admission {
    /// Space below the current spill threshold.
    Ready,
    /// Spill must catch up before another grant.
    Pause,
    /// Result disappeared, row cap reached or spilling failed.
    Stop,
}
impl ResultStore {
    /// Preserve explicit truncation when admission stops before the next append.
    pub fn mark_truncated(&self, id: &str) {
        if let Some(result) = self.shared.write().results.get_mut(id) {
            result.truncated = true;
        }
    }
    /// Inspect admission without waiting for disk; pause resumes at the ¾ target.
    #[must_use]
    pub fn admission(&self, id: &str) -> Admission {
        let inner = self.shared.read();
        let Some(result) = inner.results.get(id) else {
            return Admission::Stop;
        };
        if result.truncated {
            return Admission::Stop;
        }
        let budget = self.shared.limits.max_bytes;
        if inner.bytes > budget {
            self.shared.pressured.store(true, Ordering::SeqCst);
        }
        if !self.shared.pressured.load(Ordering::SeqCst) {
            return Admission::Ready;
        }
        if inner.bytes <= budget - budget / 4 {
            self.shared.pressured.store(false, Ordering::SeqCst);
            Admission::Ready
        } else if self.shared.can_spill() {
            Admission::Pause
        } else {
            Admission::Stop
        }
    }
    /// Wait for spill completion/release with lost-wakeup protection; cancel may drop it.
    pub async fn resume(&self, id: &str) -> Admission {
        loop {
            let changed = self.shared.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let admission = self.admission(id);
            if admission != Admission::Pause {
                return admission;
            }
            changed.await;
        }
    }
}

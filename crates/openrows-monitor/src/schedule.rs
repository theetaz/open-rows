// SPDX-License-Identifier: Apache-2.0
//! Adaptive cadence without retries or catch-up queues.
/// Per-source scheduling state; the actor alone can start reads.
#[derive(Debug)]
pub struct Cadence {
    floor: u32,
    effective: u32,
    unchanged: u8,
    due: u64,
}
impl Cadence {
    /// Start immediately, respecting the source floor.
    #[must_use]
    pub fn new(floor: u32) -> Self {
        Self {
            floor,
            effective: floor,
            unchanged: 0,
            due: 0,
        }
    }
    /// Whether this source is due; no queued missed intervals.
    #[must_use]
    pub fn due(&self, now: u64) -> bool {
        now >= self.due
    }
    /// Current effective cadence in milliseconds.
    #[must_use]
    pub fn interval_ms(&self) -> u32 {
        self.effective
    }
    /// Bound a source to one second and twenty percent of its interval.
    #[must_use]
    pub fn deadline_ms(&self) -> u32 {
        (self.effective / 5).min(1_000)
    }
    /// Complete one read and schedule from completion, avoiding overlap.
    pub fn complete(&mut self, now: u64, changed: bool, failed: bool, duration: u32) {
        self.unchanged = if changed {
            0
        } else {
            self.unchanged.saturating_add(1)
        };
        if failed || duration > self.deadline_ms() || self.unchanged >= 3 {
            self.effective = self.effective.saturating_mul(2).min(60_000);
            self.unchanged = 0;
        } else if changed {
            self.effective = self.floor;
        }
        self.due = now.saturating_add(u64::from(self.effective));
    }
    /// Delay all sources when aggregate query-time budget was exceeded.
    pub fn throttle(&mut self, now: u64) {
        self.effective = self.effective.saturating_mul(2).min(60_000);
        self.due = now.saturating_add(u64::from(self.effective));
    }
}

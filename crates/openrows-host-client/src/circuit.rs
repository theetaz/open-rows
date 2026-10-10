// SPDX-License-Identifier: Apache-2.0
//! Three abnormal exits per profile/driver in sixty seconds; no retry loop.
use crate::HostFailure;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
/// Injectable monotonic time for circuit tests.
pub trait Clock: Send + Sync {
    /// Elapsed parent time.
    fn now(&self) -> Duration;
}
/// Real monotonic process clock.
pub struct SystemClock(Instant);
impl Default for SystemClock {
    fn default() -> Self {
        Self(Instant::now())
    }
}
impl Clock for SystemClock {
    fn now(&self) -> Duration {
        self.0.elapsed()
    }
}
/// One profile/driver's abnormal-exit window.
#[derive(Clone)]
pub struct CircuitBreaker {
    clock: Arc<dyn Clock>,
    exits: Arc<Mutex<VecDeque<Duration>>>,
}
impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new(Arc::new(SystemClock::default()))
    }
}
impl CircuitBreaker {
    /// Use one clock/window for all launches of this profile/driver.
    #[must_use]
    pub fn new(clock: Arc<dyn Clock>) -> Self {
        Self {
            clock,
            exits: Arc::new(Mutex::new(VecDeque::new())),
        }
    }
    /// Refuse a fourth abnormal launch until old exits expire or explicit reset.
    /// # Errors
    /// Circuit open or corrupt state.
    pub fn check(&self) -> Result<(), HostFailure> {
        let mut exits = self.exits.lock().map_err(|_| HostFailure::Protocol)?;
        let now = self.clock.now();
        while exits
            .front()
            .is_some_and(|at| now.saturating_sub(*at) >= Duration::from_secs(60))
        {
            exits.pop_front();
        }
        if exits.len() >= 3 {
            Err(HostFailure::Spawn)
        } else {
            Ok(())
        }
    }
    /// Record one unexpected exit, once per child, after supervision completes.
    pub fn record_exit(&self) {
        if let Ok(mut exits) = self.exits.lock()
            && exits.len() < 3
        {
            exits.push_back(self.clock.now());
        }
    }
    /// Explicit user reset; never called by reconnect automatically.
    pub fn reset(&self) {
        if let Ok(mut exits) = self.exits.lock() {
            exits.clear();
        }
    }
}

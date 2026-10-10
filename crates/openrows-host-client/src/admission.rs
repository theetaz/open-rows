// SPDX-License-Identifier: Apache-2.0
//! Parent-wide reservations, released through RAII on every failure path.
use crate::{AGGREGATE_MEMORY, HOST_MEMORY, HostFailure};
use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

/// Shared budget across all supervisor profiles/drivers in one parent.
#[derive(Clone)]
pub struct MemoryBudget {
    used: Arc<AtomicUsize>,
    large: Arc<AtomicBool>,
}
impl Default for MemoryBudget {
    fn default() -> Self {
        static PARENT_BUDGET: OnceLock<MemoryBudget> = OnceLock::new();
        PARENT_BUDGET
            .get_or_init(|| Self {
                used: Arc::new(AtomicUsize::new(0)),
                large: Arc::new(AtomicBool::new(false)),
            })
            .clone()
    }
}
impl MemoryBudget {
    fn reserve(&self, bytes: usize) -> Result<(), HostFailure> {
        self.used
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |used| {
                used.checked_add(bytes)
                    .filter(|total| *total <= AGGREGATE_MEMORY)
            })
            .map(|_| ())
            .map_err(|_| HostFailure::MemoryLimit)
    }
    /// Admit one default-sized host, refusing a third active reservation.
    /// # Errors
    /// Aggregate reservation cannot fit.
    pub fn host(&self) -> Result<HostReservation, HostFailure> {
        self.reserve(HOST_MEMORY)?;
        Ok(HostReservation(self.clone()))
    }
    /// Reserve one exclusive large-row window; other large grants cannot overlap.
    /// # Errors
    /// Invalid ceiling, another large grant or aggregate exhaustion.
    pub fn large(&self, bytes: usize) -> Result<LargeReservation, HostFailure> {
        if bytes > openrows_host_protocol::LARGE_LIMIT
            || bytes == 0
            || self
                .large
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_err()
        {
            return Err(HostFailure::MemoryLimit);
        }
        if let Err(error) = self.reserve(bytes) {
            self.large.store(false, Ordering::SeqCst);
            return Err(error);
        }
        Ok(LargeReservation {
            budget: self.clone(),
            bytes,
        })
    }
    /// Current reserved process/batch bytes, not an RSS measurement.
    #[must_use]
    pub fn reserved(&self) -> usize {
        self.used.load(Ordering::SeqCst)
    }
}
/// Host reservation held until the child has been reaped.
pub struct HostReservation(MemoryBudget);
impl Drop for HostReservation {
    fn drop(&mut self) {
        self.0.used.fetch_sub(HOST_MEMORY, Ordering::SeqCst);
    }
}
/// Exclusive large-batch reservation held until sink consumption or host failure.
pub struct LargeReservation {
    budget: MemoryBudget,
    bytes: usize,
}
impl Drop for LargeReservation {
    fn drop(&mut self) {
        self.budget.used.fetch_sub(self.bytes, Ordering::SeqCst);
        self.budget.large.store(false, Ordering::SeqCst);
    }
}

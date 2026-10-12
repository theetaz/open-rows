// SPDX-License-Identifier: Apache-2.0
//! Statement generations and the fence between cancel delivery and session reuse.
use openrows_driver_api::DriverError;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use tokio::sync::{Mutex, MutexGuard};

/// Which statement a cancel may target and whether earlier delivery settled.
#[derive(Default)]
pub(super) struct State {
    /// Generation of the statement in flight; 0 while idle.
    active: AtomicU64,
    /// A request may have been sent without confirmed server closure.
    uncertain: AtomicBool,
    /// A statement was dropped before its protocol exchange drained.
    abandoned: AtomicBool,
    /// Last issued generation; held while a cancel is delivered and while a
    /// statement starts or finishes, so neither overlaps an admitted cancel.
    gate: Mutex<u64>,
}

impl State {
    /// The statement a cancel requested now belongs to; `None` while idle.
    pub(super) fn current(&self) -> Option<u64> {
        Some(self.active.load(Ordering::Acquire)).filter(|generation| *generation != 0)
    }

    /// Hold the fence for delivery while `generation` is still in flight.
    pub(super) async fn admit(
        &self,
        generation: u64,
    ) -> Result<Option<MutexGuard<'_, u64>>, DriverError> {
        let gate = self.gate.lock().await;
        if self.active.load(Ordering::Acquire) != generation {
            return Ok(None);
        }
        self.delivery_settled()?;
        Ok(Some(gate))
    }

    /// Mark that request bytes may reach the server from here on.
    pub(super) fn sending(&self) {
        self.uncertain.store(true, Ordering::Release);
    }

    /// The server closed the cancel socket, so it has processed the request.
    pub(super) fn delivered(&self) {
        self.uncertain.store(false, Ordering::Release);
    }

    /// Open the next generation once earlier cancel delivery has finished.
    pub(super) async fn begin(self: &Arc<Self>) -> Result<StatementGuard, DriverError> {
        let mut issued = self.gate.lock().await;
        self.usable()?;
        *issued = issued.wrapping_add(1).max(1);
        self.active.store(*issued, Ordering::Release);
        Ok(StatementGuard {
            state: Arc::clone(self),
            finished: false,
        })
    }

    /// Refuse reuse after an undrained statement or an unconfirmed cancel.
    fn usable(&self) -> Result<(), DriverError> {
        self.delivery_settled()?;
        if self.abandoned.load(Ordering::Acquire) {
            return Err(DriverError::Protocol(
                "a statement was abandoned before draining".to_owned(),
            ));
        }
        Ok(())
    }

    /// A cancel that may still arrive late could stop any later statement.
    fn delivery_settled(&self) -> Result<(), DriverError> {
        if self.uncertain.load(Ordering::Acquire) {
            return Err(DriverError::Protocol(
                "cancel request did not reach confirmed server closure".to_owned(),
            ));
        }
        Ok(())
    }
}

/// One statement in flight; finishing it waits for any admitted cancel.
pub(crate) struct StatementGuard {
    state: Arc<State>,
    finished: bool,
}

impl StatementGuard {
    /// End the generation under the fence; later cancel futures become no-ops.
    pub(crate) async fn finish(mut self) -> Result<(), DriverError> {
        let _gate = self.state.gate.lock().await;
        self.finished = true;
        self.state.active.store(0, Ordering::Release);
        self.state.usable()
    }
}

impl Drop for StatementGuard {
    fn drop(&mut self) {
        if !self.finished {
            // The generation stays current so the owner can still cancel the
            // server-side work of a dropped read; the session is not reused.
            self.state.abandoned.store(true, Ordering::Release);
        }
    }
}

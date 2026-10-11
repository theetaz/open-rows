// SPDX-License-Identifier: Apache-2.0
//! Permission and cancellation select before polling any network future.
use crate::AiError;
use std::{future::Future, time::Duration};
use tokio::sync::watch;

/// Cloneable cancellation without polling or background timers.
#[derive(Clone, Debug)]
pub struct Cancellation(watch::Sender<bool>);
impl Default for Cancellation {
    fn default() -> Self {
        Self(watch::channel(false).0)
    }
}
impl Cancellation {
    /// Abort all requests sharing this cancellation handle.
    pub fn cancel(&self) {
        self.0.send_replace(true);
    }
    /// Wait until this operation is cancelled.
    pub async fn cancelled(&self) {
        let mut state = self.0.subscribe();
        let _ = state.wait_for(|cancelled| *cancelled).await;
    }
}

/// Global permission; disabling invalidates all operations admitted previously.
#[derive(Clone, Debug)]
pub struct AiControl(watch::Sender<(bool, u64)>);
impl Default for AiControl {
    fn default() -> Self {
        Self(watch::channel((false, 0)).0)
    }
}
impl AiControl {
    /// Change permission and revoke admitted work when turned off.
    pub fn set_enabled(&self, enabled: bool) {
        self.0.send_modify(|state| {
            state.0 = enabled;
            if !enabled {
                state.1 += 1;
            }
        });
    }
    /// Current permission without any I/O.
    #[must_use]
    pub fn enabled(&self) -> bool {
        self.0.borrow().0
    }
    pub(crate) fn generation(&self) -> u64 {
        self.0.borrow().1
    }
    pub(crate) fn current(&self, generation: u64) -> bool {
        let state = self.0.borrow();
        state.0 && state.1 == generation
    }
    pub(crate) async fn revoked(&self, generation: u64) {
        let mut state = self.0.subscribe();
        let _ = state
            .wait_for(|value| !value.0 || value.1 != generation)
            .await;
    }
    /// Run bounded work while global AI permission remains valid.
    /// # Errors
    /// Permission revocation, cancellation, timeout or operation failure.
    pub async fn run<T>(
        &self,
        options: &RequestOptions,
        work: impl Future<Output = Result<T, AiError>>,
    ) -> Result<T, AiError> {
        let mut state = self.0.subscribe();
        let (enabled, generation) = *state.borrow();
        if !enabled {
            return Err(AiError::Disabled);
        }
        tokio::select! {
            biased;
            () = options.cancel.cancelled() => Err(AiError::Cancelled),
            _ = state.wait_for(|value| !value.0 || value.1 != generation) => Err(AiError::Disabled),
            result = tokio::time::timeout(options.timeout, work) => result.map_err(|_| AiError::Timeout)?,
        }
    }
}

/// Deadline and cancellation for the entire exchange, including sink backpressure.
#[derive(Clone, Debug)]
pub struct RequestOptions {
    /// Total deadline, not a resettable per-frame timer.
    pub timeout: Duration,
    /// Explicit cancellation handle.
    pub cancel: Cancellation,
}
impl Default for RequestOptions {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            cancel: Cancellation::default(),
        }
    }
}

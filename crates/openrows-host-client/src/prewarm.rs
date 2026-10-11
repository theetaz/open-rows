// SPDX-License-Identifier: Apache-2.0
//! One database-free startup attempt, with an asynchronous connect barrier.
use crate::HostFailure;
use std::time::{Duration, Instant};
use tokio::sync::watch;

/// Bounded diagnostic outcome; prewarm never returns a user-facing connect error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrewarmOutcome {
    /// Authenticated Probe closed and was reaped without opening a database.
    Ready,
    /// Installed resolution or private endpoint setup refused the attempt.
    Skipped(HostFailure),
    /// Admission, handshake or close failed; no retry is made.
    Failed(HostFailure),
}

/// Retained process-start measurements, containing no profile or database data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrewarmReport {
    /// Typed completion or refusal, for the app's diagnostic log.
    pub outcome: PrewarmOutcome,
    /// Resolution through shutdown/reap, measured on the parent clock.
    pub elapsed: Duration,
    /// Spawn through authenticated Ready, absent on resolution/spawn failure.
    pub handshake: Option<Duration>,
}

#[derive(Clone, Copy)]
enum State {
    Pending,
    Running,
    Finished(PrewarmReport),
}

#[derive(Clone)]
pub(crate) struct Prewarm(watch::Sender<State>);
impl Default for Prewarm {
    fn default() -> Self {
        Self(watch::channel(State::Pending).0)
    }
}
impl Prewarm {
    pub(crate) fn start(&self) -> Option<Attempt> {
        self.0
            .send_if_modified(|state| {
                if matches!(state, State::Pending) {
                    *state = State::Running;
                    true
                } else {
                    false
                }
            })
            .then(|| Attempt {
                state: self.0.clone(),
                started: Instant::now(),
            })
    }

    pub(crate) async fn wait(&self) {
        let mut observe = self.0.subscribe();
        while matches!(*observe.borrow_and_update(), State::Running) {
            if observe.changed().await.is_err() {
                break;
            }
        }
    }

    pub(crate) fn report(&self) -> Option<PrewarmReport> {
        match *self.0.borrow() {
            State::Finished(report) => Some(report),
            State::Pending | State::Running => None,
        }
    }
}

pub(crate) struct Attempt {
    state: watch::Sender<State>,
    started: Instant,
}
impl Attempt {
    pub(crate) fn finish(
        self,
        outcome: PrewarmOutcome,
        handshake: Option<Duration>,
    ) -> PrewarmReport {
        let report = PrewarmReport {
            outcome,
            elapsed: self.started.elapsed(),
            handshake,
        };
        self.state.send_replace(State::Finished(report));
        report
    }
}
impl Drop for Attempt {
    fn drop(&mut self) {
        // Cancellation must release waiters too, without retrying a partial launch.
        self.state.send_if_modified(|state| {
            if matches!(state, State::Running) {
                *state = State::Finished(PrewarmReport {
                    outcome: PrewarmOutcome::Failed(HostFailure::Exited),
                    elapsed: self.started.elapsed(),
                    handshake: None,
                });
                true
            } else {
                false
            }
        });
    }
}

#[cfg(test)]
mod tests;

// SPDX-License-Identifier: Apache-2.0
//! State shared between a session actor, its supervisor and the manager.
use crate::{CoreError, QueryEvent, SessionEvent, SessionState};
use openrows_types::{QueryId, SessionId};
use std::sync::{Mutex, MutexGuard, PoisonError};
use tokio::sync::{broadcast, mpsc::UnboundedSender, oneshot};
use tokio_util::sync::CancellationToken;

/// Commands accepted by a session actor (docs/04 §3 subset).
pub(crate) enum Command {
    /// Run one statement; progress is reported on the query's event channel.
    Execute(Queued),
    /// Close the connection and stop the actor.
    Close {
        /// Close result; dropped when the actor is already gone.
        reply: oneshot::Sender<Result<(), CoreError>>,
    },
}

/// A statement waiting in the session queue.
pub(crate) struct Queued {
    pub(crate) query: QueryId,
    pub(crate) sql: String,
    /// Positional text parameters (`$1`, …); empty for editor SQL.
    pub(crate) params: Vec<Option<String>>,
    /// Rows kept per result set; `None` leaves only the result store's caps.
    pub(crate) max_rows: Option<usize>,
    pub(crate) token: CancellationToken,
    pub(crate) events: UnboundedSender<QueryEvent>,
}

/// Lock even if another thread panicked while holding it; the data stays valid.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Session facts readable without messaging the actor.
pub(crate) struct Shared {
    pub(crate) id: SessionId,
    /// Parent of every query token; cancelled when the session closes.
    pub(crate) token: CancellationToken,
    state: Mutex<SessionState>,
    running: Mutex<Option<UnboundedSender<QueryEvent>>>,
    events: broadcast::Sender<SessionEvent>,
}

impl Shared {
    pub(crate) fn new(
        id: SessionId,
        token: CancellationToken,
        events: broadcast::Sender<SessionEvent>,
    ) -> Self {
        Self {
            id,
            token,
            state: Mutex::new(SessionState::Connecting),
            running: Mutex::new(None),
            events,
        }
    }

    pub(crate) fn state(&self) -> SessionState {
        *lock(&self.state)
    }

    /// Record and broadcast a transition; terminal states are never left.
    pub(crate) fn set_state(&self, state: SessionState, error: Option<CoreError>) {
        {
            let mut current = lock(&self.state);
            if matches!(*current, SessionState::Closed | SessionState::Failed) {
                return;
            }
            *current = state;
        }
        // No subscribers is fine; state is still readable through the manager.
        let _ = self.events.send(SessionEvent {
            session: self.id,
            state,
            error,
        });
    }

    /// Mark a statement as running so a panic can still terminate its stream.
    pub(crate) fn begin_query(&self, events: UnboundedSender<QueryEvent>) {
        *lock(&self.running) = Some(events);
        self.set_state(SessionState::Busy, None);
    }

    pub(crate) fn end_query(&self) {
        lock(&self.running).take();
    }

    /// Fail the session and the statement it was running, if any.
    pub(crate) fn fail(&self, error: CoreError) {
        if let Some(events) = lock(&self.running).take() {
            // The receiver may already be gone; nothing else to notify.
            let _ = events.send(QueryEvent::Failed {
                error: error.clone(),
            });
        }
        self.set_state(SessionState::Failed, Some(error));
    }
}

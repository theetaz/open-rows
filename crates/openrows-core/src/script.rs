// SPDX-License-Identifier: Apache-2.0
//! Multi-statement runs: pick statements from editor text, then execute them
//! one at a time on a session, stopping at the first error (docs/04 §4.5).
use crate::error::ErrorKind;
use crate::{Canceller, CoreError, QueryEvent, SessionManager};
use openrows_sql::{Dialect, split, statement_at};
use openrows_types::SessionId;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

/// Which statements of the text to run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Scope {
    /// Every statement, in order (run all, run selection).
    All,
    /// The statement under (or closest before) this byte offset.
    AtCursor(usize),
}

/// Statement texts (without `;`) selected by `scope`; empty when there is no SQL.
#[must_use]
pub fn statements(sql: &str, scope: Scope) -> Vec<String> {
    let all = split(sql, Dialect::Postgres);
    let text = |statement: &openrows_sql::Statement| statement.text(sql).to_owned();
    match scope {
        Scope::All => all.iter().map(text).collect(),
        Scope::AtCursor(cursor) => statement_at(&all, cursor)
            .and_then(|index| all.get(index))
            .map(text)
            .into_iter()
            .collect(),
    }
}

/// One query event, tagged with the index of the statement that raised it.
#[derive(Clone, Debug, PartialEq)]
pub struct ScriptEvent {
    /// Index into the run's statements.
    pub statement: usize,
    /// The statement's event.
    pub event: QueryEvent,
}

/// How a run ended.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScriptOutcome {
    /// Every statement completed.
    Completed,
    /// A statement failed; later statements did not run.
    Failed,
    /// The user cancelled; later statements did not run.
    Cancelled,
}

#[derive(Default)]
struct CancelState {
    cancelled: AtomicBool,
    current: Mutex<Option<Canceller>>,
}

/// Cancels a whole run: the running statement and every later one.
#[derive(Clone, Default)]
pub struct ScriptCanceller(Arc<CancelState>);

impl ScriptCanceller {
    /// Request cancellation; idempotent and safe from any thread.
    pub fn cancel(&self) {
        self.0.cancelled.store(true, Ordering::SeqCst);
        if let Some(current) = self.lock().as_ref() {
            current.cancel();
        }
    }

    fn is_cancelled(&self) -> bool {
        self.0.cancelled.load(Ordering::SeqCst)
    }

    /// Remember the running statement; cancel it at once if the run already was.
    fn track(&self, canceller: &Canceller) {
        *self.lock() = Some(canceller.clone());
        if self.is_cancelled() {
            canceller.cancel();
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<Canceller>> {
        self.0
            .current
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

/// Execute `statements` in order on `session`, forwarding every event to `emit`.
///
/// A statement starts only after the previous one completed. The first
/// `Failed` or `Cancelled` statement ends the run.
pub async fn run_script(
    manager: &SessionManager,
    session: SessionId,
    statements: &[String],
    cancel: &ScriptCanceller,
    mut emit: impl FnMut(ScriptEvent),
) -> ScriptOutcome {
    for (index, sql) in statements.iter().enumerate() {
        let mut send = |event| {
            emit(ScriptEvent {
                statement: index,
                event,
            });
        };
        if cancel.is_cancelled() {
            send(QueryEvent::Cancelled);
            return ScriptOutcome::Cancelled;
        }
        let outcome = match manager.execute(session, sql.as_str()) {
            Ok(mut handle) => {
                cancel.track(&handle.canceller());
                drain(&mut handle, &mut send).await
            }
            Err(error) => {
                send(QueryEvent::Failed { error });
                ScriptOutcome::Failed
            }
        };
        if outcome != ScriptOutcome::Completed {
            return outcome;
        }
    }
    ScriptOutcome::Completed
}

/// Forward one statement's events up to and including its terminal event.
async fn drain(
    handle: &mut crate::QueryHandle,
    send: &mut impl FnMut(QueryEvent),
) -> ScriptOutcome {
    while let Some(event) = handle.next_event().await {
        let outcome = match &event {
            QueryEvent::Completed { .. } => Some(ScriptOutcome::Completed),
            QueryEvent::Failed { .. } => Some(ScriptOutcome::Failed),
            QueryEvent::Cancelled => Some(ScriptOutcome::Cancelled),
            _ => None,
        };
        send(event);
        if let Some(outcome) = outcome {
            return outcome;
        }
    }
    send(QueryEvent::Failed {
        error: CoreError::new(
            ErrorKind::Internal,
            "query ended without reporting a result",
        ),
    });
    ScriptOutcome::Failed
}

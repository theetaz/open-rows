// SPDX-License-Identifier: Apache-2.0
//! Opens session actors, routes commands to them and supervises panics.
use crate::actor::{self, Connect, Mailbox};
use crate::error::ErrorKind;
use crate::shared::{Command, Queued, Shared};
use crate::{BatchSink, CoreError, QueryEvent, SessionEvent, SessionState};
use openrows_driver_api::{DriverError, Session};
use openrows_sql::select::BoundSql;
use openrows_types::{QueryId, SessionId};
use std::any::Any;
use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex, PoisonError};
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

/// Statements that may wait behind the running one in each session.
const QUEUE_CAPACITY: usize = 32;
/// Session events buffered per subscriber before the oldest are dropped.
const EVENT_CAPACITY: usize = 256;

struct Entry {
    shared: Arc<Shared>,
    commands: mpsc::Sender<Command>,
}

/// One statement to queue and where its rows go.
struct Submission {
    sql: String,
    params: Vec<Option<String>>,
    max_rows: Option<usize>,
    sink: Option<Arc<dyn BatchSink>>,
}

impl Submission {
    fn new(sql: String, params: Vec<Option<String>>) -> Self {
        Self {
            sql,
            params,
            max_rows: None,
            sink: None,
        }
    }
}

/// Cancels one query from any thread without waiting for the session actor.
#[derive(Clone, Debug)]
pub struct Canceller(CancellationToken);

impl Canceller {
    /// Request cancellation; idempotent. A queued query never starts; a running
    /// one gets an out-of-band driver cancel and ends with `Cancelled`.
    pub fn cancel(&self) {
        self.0.cancel();
    }
}

/// A submitted query: its id, ordered events and cancel handle.
pub struct QueryHandle {
    id: QueryId,
    events: mpsc::UnboundedReceiver<QueryEvent>,
    canceller: Canceller,
}

impl QueryHandle {
    /// The query's identifier.
    #[must_use]
    pub fn id(&self) -> QueryId {
        self.id
    }

    /// A cloneable handle that cancels this query.
    #[must_use]
    pub fn canceller(&self) -> Canceller {
        self.canceller.clone()
    }

    /// Next event; `None` after the terminal event has been received.
    pub async fn next_event(&mut self) -> Option<QueryEvent> {
        self.events.recv().await
    }
}

/// Owns every live session; one tokio task (actor) per session (ADR-0007).
///
/// Must be used inside a tokio runtime. Dropping the manager closes all sessions.
pub struct SessionManager {
    sessions: Mutex<HashMap<SessionId, Entry>>,
    store: Arc<dyn BatchSink>,
    events: broadcast::Sender<SessionEvent>,
    root: CancellationToken,
}

impl SessionManager {
    /// Create a manager whose sessions write result batches to `store`.
    #[must_use]
    pub fn new(store: Arc<dyn BatchSink>) -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            store,
            events: broadcast::channel(EVENT_CAPACITY).0,
            root: CancellationToken::new(),
        }
    }

    /// Session state transitions for every session, including panics.
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<SessionEvent> {
        self.events.subscribe()
    }

    /// Current state, or `None` when the id is unknown or already closed.
    #[must_use]
    pub fn state(&self, session: SessionId) -> Option<SessionState> {
        self.with_entry(session, |entry| entry.shared.state())
    }

    /// Spawn a session actor that first awaits `connect`, then serves commands.
    ///
    /// # Errors
    /// The connect failure, or `Internal` when the driver panicked while connecting.
    pub async fn open<F>(&self, connect: F) -> Result<SessionId, CoreError>
    where
        F: Future<Output = Result<Box<dyn Session>, DriverError>> + Send + 'static,
    {
        let id = SessionId::new();
        let shared = Arc::new(Shared::new(
            id,
            self.root.child_token(),
            self.events.clone(),
        ));
        shared.set_state(SessionState::Connecting, None);
        let (commands, receiver) = mpsc::channel(QUEUE_CAPACITY);
        let (ready, connected) = oneshot::channel();
        let mail = Mailbox {
            shared: shared.clone(),
            commands: receiver,
            store: self.store.clone(),
        };
        let connect: Connect = Box::pin(connect);
        let task = tokio::spawn(actor::run(connect, mail, ready));
        tokio::spawn(supervise(task, shared.clone()));
        self.lock().insert(id, Entry { shared, commands });
        let result = connected.await.unwrap_or_else(|_| {
            Err(CoreError::new(
                ErrorKind::Internal,
                "session task ended while connecting",
            ))
        });
        if result.is_err() {
            self.lock().remove(&id);
        }
        result.map(|()| id)
    }

    /// Queue `sql` on `session`; events and the result arrive on the handle.
    ///
    /// # Errors
    /// `UnknownSession`, `SessionClosed` for failed sessions, or `QueueFull`.
    pub fn execute(
        &self,
        session: SessionId,
        sql: impl Into<String>,
    ) -> Result<QueryHandle, CoreError> {
        self.submit(session, Submission::new(sql.into(), Vec::new()))
    }

    /// Queue `sql` with positional text parameters (`$1`, …; `None` is NULL).
    ///
    /// # Errors
    /// As [`SessionManager::execute`].
    pub fn execute_bound(
        &self,
        session: SessionId,
        sql: impl Into<String>,
        params: Vec<Option<String>>,
    ) -> Result<QueryHandle, CoreError> {
        self.submit(session, Submission::new(sql.into(), params))
    }

    /// Queue a bound statement keeping at most `max_rows` rows per result
    /// set (the result ends `truncated` when more rows existed); an empty
    /// parameter list runs the text as written.
    ///
    /// # Errors
    /// As [`SessionManager::execute`].
    pub fn execute_statement(
        &self,
        session: SessionId,
        statement: BoundSql,
        max_rows: Option<usize>,
    ) -> Result<QueryHandle, CoreError> {
        let submission = Submission {
            max_rows,
            ..Submission::new(statement.sql, statement.params)
        };
        self.submit(session, submission)
    }

    /// Queue a bound statement whose batches go to `sink` instead of the
    /// manager's store (export streams rows straight to a file).
    ///
    /// # Errors
    /// As [`SessionManager::execute`].
    pub fn execute_into(
        &self,
        session: SessionId,
        statement: BoundSql,
        sink: Arc<dyn BatchSink>,
    ) -> Result<QueryHandle, CoreError> {
        let submission = Submission {
            sink: Some(sink),
            ..Submission::new(statement.sql, statement.params)
        };
        self.submit(session, submission)
    }

    fn submit(&self, session: SessionId, submission: Submission) -> Result<QueryHandle, CoreError> {
        let (sender, events) = mpsc::unbounded_channel();
        let (commands, token) = self
            .with_entry(session, |entry| {
                (entry.commands.clone(), entry.shared.token.child_token())
            })
            .ok_or_else(|| CoreError::unknown_session(session))?;
        let id = QueryId::new();
        let queued = Queued {
            query: id,
            sql: submission.sql,
            params: submission.params,
            max_rows: submission.max_rows,
            sink: submission.sink,
            token: token.clone(),
            events: sender,
        };
        commands
            .try_send(Command::Execute(queued))
            .map_err(|error| match error {
                mpsc::error::TrySendError::Full(_) => CoreError::new(
                    ErrorKind::QueueFull,
                    format!("session {session} queue is full"),
                ),
                mpsc::error::TrySendError::Closed(_) => CoreError::session_closed(session),
            })?;
        Ok(QueryHandle {
            id,
            events,
            canceller: Canceller(token),
        })
    }

    /// Cancel the session's queries, close the connection and forget the session.
    ///
    /// # Errors
    /// `UnknownSession`, or the driver's failure while terminating the connection.
    pub async fn close(&self, session: SessionId) -> Result<(), CoreError> {
        let entry = self
            .lock()
            .remove(&session)
            .ok_or_else(|| CoreError::unknown_session(session))?;
        entry.shared.token.cancel();
        let (reply, closed) = oneshot::channel();
        if entry.commands.send(Command::Close { reply }).await.is_err() {
            return Ok(());
        }
        closed.await.unwrap_or(Ok(()))
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<SessionId, Entry>> {
        self.sessions.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn with_entry<T>(&self, session: SessionId, read: impl FnOnce(&Entry) -> T) -> Option<T> {
        self.lock().get(&session).map(read)
    }
}

impl Drop for SessionManager {
    fn drop(&mut self) {
        self.root.cancel();
    }
}

/// Turn an actor panic into a `Failed` session instead of a crashed app (NFR-REL-02).
async fn supervise(task: JoinHandle<()>, shared: Arc<Shared>) {
    let Err(error) = task.await else {
        return;
    };
    let message = if error.is_panic() {
        format!(
            "session driver panicked: {}",
            panic_message(error.into_panic())
        )
    } else {
        "session task was aborted".to_owned()
    };
    shared.fail(CoreError::new(ErrorKind::Internal, message));
}

fn panic_message(payload: Box<dyn Any + Send>) -> String {
    match payload.downcast::<String>() {
        Ok(message) => *message,
        Err(payload) => payload.downcast_ref::<&str>().map_or_else(
            || "unknown panic".to_owned(),
            |message| (*message).to_owned(),
        ),
    }
}

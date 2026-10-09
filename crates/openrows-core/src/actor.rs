// SPDX-License-Identifier: Apache-2.0
//! The session actor: sole owner of one driver connection (ADR-0007).
use crate::error::{ErrorKind, keeps_session};
use crate::resilience::{Link, Recovery, lost_in_flight, transaction_lost};
use crate::shared::{Command, Queued, Shared};
use crate::sink::QuerySink;
use crate::{BatchSink, CoreError, QueryEvent, SessionState};
use openrows_driver_api::{BoxFuture, CancelHandle, DriverError, ExecSummary, ResultSink, Session};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;

/// How long a server-side cancel may take before the connection is discarded.
const CANCEL_GRACE: Duration = Duration::from_secs(2);

/// Future that produces a connected driver session.
pub(crate) type Connect = BoxFuture<'static, Result<Box<dyn Session>, DriverError>>;

type CloseReply = oneshot::Sender<Result<(), CoreError>>;

/// How one statement ended, before it is reported.
enum Outcome {
    Done(Result<ExecSummary, DriverError>),
    Cancelled {
        usable: bool,
    },
    /// Cancelled because it ran past `limit` (the statement timeout).
    TimedOut {
        usable: bool,
        limit: Duration,
    },
}

/// Why a running statement was stopped.
enum Stop {
    Cancelled,
    TimedOut(Duration),
}

/// Why the actor stopped serving statements on its connection.
pub(crate) enum Exit {
    /// `Close`: reply once the connection is closed.
    Close(CloseReply),
    /// Every sender is gone, or the session was closed while reconnecting.
    Gone,
    /// Idle long enough: close the connection until the next statement.
    Suspend,
}

/// Whether the actor keeps serving after a step.
enum Flow {
    Continue,
    /// The session was closed while reconnecting.
    Closed,
}

/// What woke the idle actor.
enum Next {
    Command(Option<Command>),
    Probe,
}

/// Everything an actor needs besides its driver session.
pub(crate) struct Mailbox {
    pub(crate) shared: Arc<Shared>,
    pub(crate) commands: mpsc::Receiver<Command>,
    pub(crate) store: Arc<dyn BatchSink>,
    /// Keepalive and reconnect recipe; `None` fails the session on loss.
    pub(crate) link: Option<Link>,
}

pub(crate) struct Actor {
    pub(crate) mail: Mailbox,
    session: Box<dyn Session>,
    cancel: Option<Arc<dyn CancelHandle>>,
    /// When the last statement ended (or the connection opened).
    used: Instant,
    /// A statement to run before reading the queue (the one that woke a
    /// suspended session).
    pending: Option<Queued>,
}

/// Connect, report readiness, then serve commands until closed or broken.
pub(crate) async fn run(
    connect: Connect,
    mail: Mailbox,
    ready: oneshot::Sender<Result<(), CoreError>>,
) {
    let session = match connect.await {
        Ok(session) => session,
        Err(error) => {
            let error = CoreError::from(&error);
            mail.shared.fail(error.clone());
            // `open` may have stopped waiting; the state already says Failed.
            let _ = ready.send(Err(error));
            return;
        }
    };
    mail.shared.set_state(SessionState::Idle, None);
    let _ = ready.send(Ok(()));
    Actor::new(mail, session, None).serve().await;
}

impl Actor {
    /// An actor on a fresh connection, first running `pending` if given.
    pub(crate) fn new(mail: Mailbox, session: Box<dyn Session>, pending: Option<Queued>) -> Self {
        Self {
            mail,
            cancel: session.cancel_handle(),
            session,
            used: Instant::now(),
            pending,
        }
    }

    async fn serve(mut self) {
        loop {
            match self.serve_queries().await {
                Err(error) => {
                    self.mail.shared.fail(error.clone());
                    fail_waiting(&mut self.mail, &error);
                    return;
                }
                // `close` may have stopped waiting; the state says Closed.
                Ok(Exit::Close(reply)) => return drop(reply.send(self.close().await)),
                Ok(Exit::Gone) => return drop(self.close().await),
                Ok(Exit::Suspend) => match crate::dormant::sleep(self).await {
                    Some(actor) => self = actor,
                    None => return,
                },
            }
        }
    }

    /// Run statements until `Close`, no senders, an idle suspend or a broken connection.
    async fn serve_queries(&mut self) -> Result<Exit, CoreError> {
        loop {
            let next = if let Some(queued) = self.pending.take() {
                Next::Command(Some(Command::Execute(queued)))
            } else {
                tokio::select! {
                    biased;
                    command = self.mail.commands.recv() => Next::Command(command),
                    () = probe_due(self.mail.link.as_mut(), &self.mail.shared) => Next::Probe,
                }
            };
            let flow = match next {
                Next::Probe => self.check().await?,
                Next::Command(Some(Command::Execute(queued))) => self.run(queued).await?,
                Next::Command(Some(Command::Suspend { idle })) if self.may_suspend(idle) => {
                    return Ok(Exit::Suspend);
                }
                Next::Command(Some(Command::Suspend { .. })) => Flow::Continue,
                Next::Command(Some(Command::Close { reply })) => return Ok(Exit::Close(reply)),
                Next::Command(None) => return Ok(Exit::Gone),
            };
            if let Flow::Closed = flow {
                return Ok(Exit::Gone);
            }
        }
    }

    /// Only a resilient session can reopen its connection; a transaction
    /// would be lost with it.
    fn may_suspend(&self, idle: Duration) -> bool {
        self.mail.link.is_some() && !self.session.in_transaction() && self.used.elapsed() >= idle
    }

    /// Close the connection, keeping the mailbox for the suspended session.
    pub(crate) async fn disconnect(self) -> Mailbox {
        // A goodbye is best effort: the connection is dropped either way.
        let _ = self.session.close().await;
        self.mail
    }

    async fn close(self) -> Result<(), CoreError> {
        let result = self.session.close().await;
        self.mail.shared.set_state(SessionState::Closed, None);
        result.map_err(|error| CoreError::from(&error))
    }

    /// Probe a connection that idled long enough to have died unnoticed,
    /// then run the statement; a loss while it runs is reported, not retried.
    async fn run(&mut self, queued: Queued) -> Result<Flow, CoreError> {
        if self.mail.link.as_ref().is_some_and(Link::is_stale) {
            let checked = self.check().await;
            if !matches!(checked, Ok(Flow::Continue)) {
                let error = match &checked {
                    Err(error) => error.clone(),
                    Ok(_) => CoreError::session_closed(self.mail.shared.id),
                };
                let _ = queued.events.send(QueryEvent::Failed { error });
                return checked;
            }
        }
        match self.execute(queued).await {
            Ok(()) => Ok(Flow::Continue),
            Err(Broken::Lost(cause)) => self.recover(cause).await,
            Err(Broken::Unusable(error)) => Err(error),
        }
    }

    /// Ping the idle connection; replace it when it no longer answers.
    async fn check(&mut self) -> Result<Flow, CoreError> {
        let Some(link) = self.mail.link.as_mut() else {
            return Ok(Flow::Continue);
        };
        match link.ping(self.session.as_mut()).await {
            None => {
                link.touch();
                Ok(Flow::Continue)
            }
            Some(cause) => self.recover(CoreError::from(&cause)).await,
        }
    }

    /// Reconnect after a lost connection, unless it held a transaction.
    async fn recover(&mut self, cause: CoreError) -> Result<Flow, CoreError> {
        if self.session.in_transaction() {
            return Err(transaction_lost(&cause));
        }
        let Some(link) = self.mail.link.as_ref() else {
            return Err(cause);
        };
        let shared = &self.mail.shared;
        shared.set_state(SessionState::Reconnecting, Some(cause));
        match link.restore(&shared.token).await {
            Recovery::Restored(session) => {
                self.cancel = session.cancel_handle();
                // The old connection is dead; dropping it closes its socket.
                drop(std::mem::replace(&mut self.session, session));
                if let Some(link) = self.mail.link.as_mut() {
                    link.touch();
                }
                self.mail.shared.set_state(SessionState::Idle, None);
                Ok(Flow::Continue)
            }
            Recovery::Closed => Ok(Flow::Closed),
            Recovery::Failed(error) => Err(error),
        }
    }

    /// Run one statement; `Err` means the connection is no longer usable.
    async fn execute(&mut self, queued: Queued) -> Result<(), Broken> {
        let emit = |event| {
            let _ = queued.events.send(event);
        };
        if queued.token.is_cancelled() {
            emit(QueryEvent::Cancelled);
            return Ok(());
        }
        self.mail.shared.begin_query(queued.events.clone());
        emit(QueryEvent::Started {
            query: queued.query,
        });
        let started = Instant::now();
        let store = queued
            .sink
            .clone()
            .unwrap_or_else(|| self.mail.store.clone());
        let mut sink = QuerySink::new(queued.events.clone(), store).with_max_rows(queued.max_rows);
        let outcome = run_statement(
            self.session.as_mut(),
            self.cancel.as_ref(),
            &queued,
            &mut sink,
        )
        .await;
        let in_transaction = self.session.in_transaction();
        self.mail.shared.set_in_transaction(in_transaction);
        let (terminal, usable) = report(outcome, &mut sink, (started.elapsed(), in_transaction));
        self.mail.shared.end_query();
        self.used = Instant::now();
        if usable.is_ok() {
            if let Some(link) = self.mail.link.as_mut() {
                link.touch();
                link.remember(&queued.sql, in_transaction);
            }
            self.mail.shared.set_state(SessionState::Idle, None);
        }
        // The session is Idle before the caller hears the statement ended.
        emit(terminal);
        usable
    }
}

/// Statements still queued when the session failed never ran: say so.
pub(crate) fn fail_waiting(mail: &mut Mailbox, error: &CoreError) {
    mail.commands.close();
    while let Ok(command) = mail.commands.try_recv() {
        if let Command::Execute(queued) = command {
            let _ = queued.events.send(QueryEvent::Failed {
                error: error.clone(),
            });
        }
    }
}

/// Why the connection cannot run the next statement.
enum Broken {
    /// The transport dropped; a resilient session reconnects.
    Lost(CoreError),
    /// Anything else (protocol violation, unacknowledged cancel).
    Unusable(CoreError),
}

/// Resolves when an idle resilient session should be probed; never for others.
async fn probe_due(link: Option<&mut Link>, shared: &Shared) {
    match link {
        Some(link) => link.probe_due(&shared.wake).await,
        None => std::future::pending().await,
    }
}

/// Execute, racing the query's cancel token; cancel goes to the server out of band.
async fn run_statement(
    session: &mut dyn Session,
    cancel: Option<&Arc<dyn CancelHandle>>,
    queued: &Queued,
    sink: &mut dyn ResultSink,
) -> Outcome {
    let mut exec = session.execute_bound(&queued.sql, &queued.params, sink);
    let stop = tokio::select! {
        biased;
        result = &mut exec => return Outcome::Done(result),
        () = queued.token.cancelled() => Stop::Cancelled,
        () = expire(queued.timeout) => Stop::TimedOut(queued.timeout.unwrap_or_default()),
    };
    let usable = match cancel {
        None => false,
        Some(cancel) => {
            // Detached on purpose: the request must not wait for, or block, the actor.
            drop(tokio::spawn(cancel.cancel()));
            match tokio::time::timeout(CANCEL_GRACE, exec).await {
                Ok(Ok(summary)) => return Outcome::Done(Ok(summary)),
                Ok(Err(error)) => keeps_session(&error),
                Err(_) => false,
            }
        }
    };
    match stop {
        Stop::Cancelled => Outcome::Cancelled { usable },
        Stop::TimedOut(limit) => Outcome::TimedOut { usable, limit },
    }
}

/// Resolves once `timeout` passed; never without one.
async fn expire(timeout: Option<Duration>) {
    match timeout {
        Some(timeout) => tokio::time::sleep(timeout).await,
        None => std::future::pending().await,
    }
}

/// The session must be discarded: a cancel the server never acknowledged.
fn unacknowledged() -> Broken {
    Broken::Unusable(CoreError::new(
        ErrorKind::SessionClosed,
        "connection discarded after cancel without server acknowledgement",
    ))
}

/// The terminal event to emit, and `Err` with why the session must be
/// discarded. `context` is the elapsed time and whether a transaction was open.
fn report(
    outcome: Outcome,
    sink: &mut QuerySink,
    (elapsed, in_transaction): (Duration, bool),
) -> (QueryEvent, Result<(), Broken>) {
    match outcome {
        Outcome::Done(Ok(summary)) => {
            sink.finish(&summary);
            (QueryEvent::Completed { elapsed }, Ok(()))
        }
        Outcome::Done(Err(error)) if error.is_connection_lost() => {
            sink.end_result();
            let event = QueryEvent::Failed {
                error: lost_in_flight(&error, in_transaction),
            };
            (event, Err(Broken::Lost(CoreError::from(&error))))
        }
        Outcome::Done(Err(error)) => {
            sink.end_result();
            let core = CoreError::from(&error);
            let event = QueryEvent::Failed {
                error: core.clone(),
            };
            if keeps_session(&error) {
                (event, Ok(()))
            } else {
                (event, Err(Broken::Unusable(core)))
            }
        }
        Outcome::Cancelled { usable } => {
            sink.end_result();
            (QueryEvent::Cancelled, acknowledged(usable))
        }
        Outcome::TimedOut { usable, limit } => {
            sink.end_result();
            let event = QueryEvent::Failed {
                error: CoreError::statement_timeout(limit),
            };
            (event, acknowledged(usable))
        }
    }
}

/// `Ok` when the server acknowledged the cancel and the session is usable.
fn acknowledged(usable: bool) -> Result<(), Broken> {
    if usable {
        Ok(())
    } else {
        Err(unacknowledged())
    }
}

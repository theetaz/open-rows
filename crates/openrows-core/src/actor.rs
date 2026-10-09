// SPDX-License-Identifier: Apache-2.0
//! The session actor: sole owner of one driver connection (ADR-0007).
use crate::error::{ErrorKind, keeps_session};
use crate::resilience::{Link, Recovery, lost_in_flight, transaction_lost};
use crate::shared::{Command, Queued, Shared};
use crate::sink::QuerySink;
use crate::{BatchSink, CoreError, QueryEvent, SessionState};
use openrows_driver_api::{BoxFuture, CancelHandle, DriverError, ExecSummary, ResultSink, Session};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot};

/// How long a server-side cancel may take before the connection is discarded.
const CANCEL_GRACE: Duration = Duration::from_secs(2);

/// Future that produces a connected driver session.
pub(crate) type Connect = BoxFuture<'static, Result<Box<dyn Session>, DriverError>>;

type CloseReply = oneshot::Sender<Result<(), CoreError>>;

/// How one statement ended, before it is reported.
enum Outcome {
    Done(Result<ExecSummary, DriverError>),
    Cancelled { usable: bool },
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

struct Actor {
    mail: Mailbox,
    session: Box<dyn Session>,
    cancel: Option<Arc<dyn CancelHandle>>,
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
    let cancel = session.cancel_handle();
    Actor {
        mail,
        session,
        cancel,
    }
    .serve()
    .await;
}

impl Actor {
    async fn serve(mut self) {
        match self.serve_queries().await {
            Err(error) => {
                self.mail.shared.fail(error.clone());
                self.fail_waiting(&error);
            }
            // `close` may have stopped waiting; the state says Closed.
            Ok(Some(reply)) => drop(reply.send(self.close().await)),
            // Every sender is gone (the manager was dropped), or the session
            // was closed while reconnecting.
            Ok(None) => drop(self.close().await),
        }
    }

    /// Run statements until `Close` (its reply), no senders (`None`) or a broken connection.
    async fn serve_queries(&mut self) -> Result<Option<CloseReply>, CoreError> {
        loop {
            let next = tokio::select! {
                biased;
                command = self.mail.commands.recv() => Next::Command(command),
                () = probe_due(self.mail.link.as_mut(), &self.mail.shared) => Next::Probe,
            };
            let flow = match next {
                Next::Probe => self.check().await?,
                Next::Command(Some(Command::Execute(queued))) => self.run(queued).await?,
                Next::Command(Some(Command::Close { reply })) => return Ok(Some(reply)),
                Next::Command(None) => return Ok(None),
            };
            if let Flow::Closed = flow {
                return Ok(None);
            }
        }
    }

    async fn close(self) -> Result<(), CoreError> {
        let result = self.session.close().await;
        self.mail.shared.set_state(SessionState::Closed, None);
        result.map_err(|error| CoreError::from(&error))
    }

    /// Statements still queued when the session failed never ran: say so.
    fn fail_waiting(&mut self, error: &CoreError) {
        self.mail.commands.close();
        while let Ok(command) = self.mail.commands.try_recv() {
            if let Command::Execute(queued) = command {
                let _ = queued.events.send(QueryEvent::Failed {
                    error: error.clone(),
                });
            }
        }
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
        let usable = report(
            outcome,
            &mut sink,
            (started.elapsed(), in_transaction),
            emit,
        );
        self.mail.shared.end_query();
        usable?;
        if let Some(link) = self.mail.link.as_mut() {
            link.touch();
            link.remember(&queued.sql, in_transaction);
        }
        self.mail.shared.set_state(SessionState::Idle, None);
        Ok(())
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
    tokio::select! {
        biased;
        result = &mut exec => return Outcome::Done(result),
        () = queued.token.cancelled() => {}
    }
    let Some(cancel) = cancel else {
        return Outcome::Cancelled { usable: false };
    };
    // Detached on purpose: the request must not wait for, or block, the actor.
    drop(tokio::spawn(cancel.cancel()));
    match tokio::time::timeout(CANCEL_GRACE, exec).await {
        Ok(Ok(summary)) => Outcome::Done(Ok(summary)),
        Ok(Err(error)) => Outcome::Cancelled {
            usable: keeps_session(&error),
        },
        Err(_) => Outcome::Cancelled { usable: false },
    }
}

/// Emit the terminal events; `Err` carries why the session must be discarded.
/// `context` is the elapsed time and whether a transaction was open.
fn report(
    outcome: Outcome,
    sink: &mut QuerySink,
    (elapsed, in_transaction): (Duration, bool),
    emit: impl Fn(QueryEvent),
) -> Result<(), Broken> {
    match outcome {
        Outcome::Done(Ok(summary)) => {
            sink.finish(&summary);
            emit(QueryEvent::Completed { elapsed });
            Ok(())
        }
        Outcome::Done(Err(error)) if error.is_connection_lost() => {
            sink.end_result();
            emit(QueryEvent::Failed {
                error: lost_in_flight(&error, in_transaction),
            });
            Err(Broken::Lost(CoreError::from(&error)))
        }
        Outcome::Done(Err(error)) => {
            sink.end_result();
            let core = CoreError::from(&error);
            emit(QueryEvent::Failed {
                error: core.clone(),
            });
            if keeps_session(&error) {
                Ok(())
            } else {
                Err(Broken::Unusable(core))
            }
        }
        Outcome::Cancelled { usable } => {
            sink.end_result();
            emit(QueryEvent::Cancelled);
            if usable {
                Ok(())
            } else {
                Err(Broken::Unusable(CoreError::new(
                    ErrorKind::SessionClosed,
                    "connection discarded after cancel without server acknowledgement",
                )))
            }
        }
    }
}

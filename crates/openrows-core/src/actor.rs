// SPDX-License-Identifier: Apache-2.0
//! The session actor: sole owner of one driver connection (ADR-0007).
use crate::error::{ErrorKind, keeps_session};
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

/// Everything an actor needs besides its driver session.
pub(crate) struct Mailbox {
    pub(crate) shared: Arc<Shared>,
    pub(crate) commands: mpsc::Receiver<Command>,
    pub(crate) store: Arc<dyn BatchSink>,
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
            Err(error) => self.mail.shared.fail(error),
            // `close` may have stopped waiting; the state says Closed.
            Ok(Some(reply)) => drop(reply.send(self.close().await)),
            // Every sender is gone: the manager was dropped.
            Ok(None) => drop(self.close().await),
        }
    }

    /// Run statements until `Close` (its reply), no senders (`None`) or a broken connection.
    async fn serve_queries(&mut self) -> Result<Option<CloseReply>, CoreError> {
        while let Some(command) = self.mail.commands.recv().await {
            match command {
                Command::Execute(queued) => self.execute(queued).await?,
                Command::Close { reply } => return Ok(Some(reply)),
            }
        }
        Ok(None)
    }

    async fn close(self) -> Result<(), CoreError> {
        let result = self.session.close().await;
        self.mail.shared.set_state(SessionState::Closed, None);
        result.map_err(|error| CoreError::from(&error))
    }

    /// Run one statement; `Err` means the connection is no longer usable.
    async fn execute(&mut self, queued: Queued) -> Result<(), CoreError> {
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
        let mut sink = QuerySink::new(queued.events.clone(), self.mail.store.clone());
        let outcome = run_statement(
            self.session.as_mut(),
            self.cancel.as_ref(),
            &queued,
            &mut sink,
        )
        .await;
        let usable = report(outcome, &mut sink, started.elapsed(), emit);
        self.mail.shared.end_query();
        match usable {
            Ok(()) => {
                self.mail.shared.set_state(SessionState::Idle, None);
                Ok(())
            }
            Err(error) => Err(error),
        }
    }
}

/// Execute, racing the query's cancel token; cancel goes to the server out of band.
async fn run_statement(
    session: &mut dyn Session,
    cancel: Option<&Arc<dyn CancelHandle>>,
    queued: &Queued,
    sink: &mut dyn ResultSink,
) -> Outcome {
    let mut exec = session.execute(&queued.sql, sink);
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
fn report(
    outcome: Outcome,
    sink: &mut QuerySink,
    elapsed: Duration,
    emit: impl Fn(QueryEvent),
) -> Result<(), CoreError> {
    match outcome {
        Outcome::Done(Ok(summary)) => {
            sink.finish(&summary);
            emit(QueryEvent::Completed { elapsed });
            Ok(())
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
                Err(core)
            }
        }
        Outcome::Cancelled { usable } => {
            sink.end_result();
            emit(QueryEvent::Cancelled);
            if usable {
                Ok(())
            } else {
                Err(CoreError::new(
                    ErrorKind::SessionClosed,
                    "connection discarded after cancel without server acknowledgement",
                ))
            }
        }
    }
}

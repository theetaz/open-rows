// SPDX-License-Identifier: Apache-2.0
//! Keepalive probes and transparent reconnects of idle sessions (M1-08,
//! docs/14 §2, docs/24 §6).
//!
//! A resilient session keeps the recipe to open its connection again. While
//! idle it pings the server every keepalive interval, and at once when the
//! manager hears that the machine woke or the network changed; a statement
//! that arrives after a longer pause is preceded by a ping. A connection that
//! no longer answers is replaced with backoff and the statements that set the
//! search path or database are replayed. Two things are never done behind the
//! user's back: a statement already sent is not run again, and a session
//! whose server held a transaction open is not replaced (that transaction is
//! gone, so the session fails instead of carrying on without it).
use crate::{CoreError, ErrorKind};
use openrows_driver_api::{BoxFuture, DiscardSink, DriverError, Session};
use openrows_sql::{ContextSetting, Dialect, session_context};
use std::future::Future;
use std::io;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Notify, watch};
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

/// Opens the session's connection again: same route, database and options.
pub type Reconnect =
    Arc<dyn Fn() -> BoxFuture<'static, Result<Box<dyn Session>, DriverError>> + Send + Sync>;

/// Probe and reconnect timing; defaults follow docs/14 §2 and §4.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetryPolicy {
    /// How long a keepalive probe may take before the connection counts as lost.
    pub probe_timeout: Duration,
    /// Idle time after which a statement is preceded by a probe.
    pub stale_after: Duration,
    /// Wait before the second reconnect attempt; doubles per attempt.
    pub first_delay: Duration,
    /// Longest wait between attempts.
    pub max_delay: Duration,
    /// Reconnect attempts before the session fails.
    pub attempts: u32,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            probe_timeout: Duration::from_secs(10),
            stale_after: Duration::from_secs(10),
            first_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(30),
            attempts: 5,
        }
    }
}

/// Everything a session needs to survive a lost connection.
#[derive(Clone)]
pub struct Resilience {
    /// Opens the connection, first and after every loss.
    pub reconnect: Reconnect,
    /// Dialect whose search-path and database statements are replayed after
    /// a reconnect; `None` replays nothing.
    pub dialect: Option<Dialect>,
    /// Probe and reconnect timing.
    pub policy: RetryPolicy,
    /// Statements run on every new connection, first and after each
    /// reconnect, before the replayed settings: the server-side statement
    /// timeout (M1-01). A server that rejects one (e.g. a `MySQL` without
    /// `max_execution_time`) keeps the connection; only the client-side
    /// limit applies then.
    pub setup: Vec<String>,
}

/// Connect with `recipe` for the first time and run its setup statements.
pub(crate) fn first_connect(recipe: &Resilience) -> crate::actor::Connect {
    let reconnect = Arc::clone(&recipe.reconnect);
    let setup = recipe.setup.clone();
    Box::pin(async move {
        let mut session = reconnect().await?;
        apply_setup(session.as_mut(), &setup).await?;
        Ok(session)
    })
}

/// Run setup statements; a statement the server rejects is skipped (the
/// client-side limit still applies), a lost connection is an error.
async fn apply_setup(session: &mut dyn Session, setup: &[String]) -> Result<(), DriverError> {
    for sql in setup {
        match session.execute(sql, &mut DiscardSink).await {
            Ok(_) => {}
            Err(DriverError::Server { code, .. }) => {
                tracing::warn!(code = %code, "session setup statement rejected by the server");
            }
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

/// How restoring a lost connection ended.
pub(crate) enum Recovery {
    /// A new connection with the session settings replayed.
    Restored(Box<dyn Session>),
    /// The session was closed while reconnecting.
    Closed,
    /// No attempt succeeded, or one failed in a way retrying cannot fix.
    Failed(CoreError),
}

/// Why one reconnect attempt failed.
enum AttemptError {
    /// Transient (network, server starting up): try again after the backoff.
    Retry(DriverError),
    /// Retrying cannot help (credentials, TLS, a setting that no longer applies).
    Stop(CoreError),
}

/// A resilient session's recipe, replayed settings and idle clock.
pub(crate) struct Link {
    recipe: Resilience,
    replay: Vec<(ContextSetting, String)>,
    keepalive: watch::Receiver<Option<Duration>>,
    idle_since: Instant,
}

impl Link {
    pub(crate) fn new(recipe: Resilience, keepalive: watch::Receiver<Option<Duration>>) -> Self {
        Self {
            recipe,
            replay: Vec::new(),
            keepalive,
            idle_since: Instant::now(),
        }
    }

    /// The connection just answered; the idle clock starts again.
    pub(crate) fn touch(&mut self) {
        self.idle_since = Instant::now();
    }

    /// Remember `sql` for replay when it set the search path or database
    /// outside a transaction (inside one it may still be rolled back).
    pub(crate) fn remember(&mut self, sql: &str, in_transaction: bool) {
        let Some(dialect) = self.recipe.dialect else {
            return;
        };
        let Some(setting) = session_context(sql, dialect).filter(|_| !in_transaction) else {
            return;
        };
        self.replay.retain(|(kept, _)| *kept != setting);
        self.replay.push((setting, sql.to_owned()));
    }

    /// True when keepalive is on and the session idled long enough that its
    /// connection may have died unnoticed.
    pub(crate) fn is_stale(&self) -> bool {
        let threshold = |interval: Duration| interval.min(self.recipe.policy.stale_after);
        (*self.keepalive.borrow())
            .is_some_and(|interval| self.idle_since.elapsed() >= threshold(interval))
    }

    /// Resolves when the idle session should be probed: the keepalive
    /// interval passed since the last activity, or `wake` was notified.
    pub(crate) async fn probe_due(&mut self, wake: &Notify) {
        loop {
            let interval = *self.keepalive.borrow_and_update();
            let deadline = interval.map(|interval| self.idle_since + interval);
            tokio::select! {
                () = wake.notified() => return,
                () = sleep_until(deadline) => return,
                changed = self.keepalive.changed() => {
                    if changed.is_err() {
                        // The manager is gone; only a wake can still ask.
                        return wake.notified().await;
                    }
                }
            }
        }
    }

    /// `None` when the connection answered a ping in time (a server error,
    /// e.g. inside an aborted transaction, also proves it is alive), else why
    /// it counts as lost.
    pub(crate) async fn ping(&self, session: &mut dyn Session) -> Option<DriverError> {
        let limit = self.recipe.policy.probe_timeout;
        match tokio::time::timeout(limit, session.ping()).await {
            Ok(Ok(()) | Err(DriverError::Server { .. })) => None,
            Ok(Err(error)) => Some(error),
            Err(_) => Some(DriverError::Network(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("the server did not answer a keepalive probe within {limit:?}"),
            ))),
        }
    }

    /// Reconnect with backoff (docs/14 §2: 1 s, 2 s, 4 s … at most 30 s)
    /// and replay the session settings; stops when `closing` is cancelled.
    pub(crate) async fn restore(&self, closing: &CancellationToken) -> Recovery {
        let policy = self.recipe.policy;
        let mut delay = policy.first_delay;
        let mut last = None;
        for attempt in 0..policy.attempts.max(1) {
            let wait = (attempt > 0).then_some(delay);
            if attempt > 0 {
                delay = delay.saturating_mul(2).min(policy.max_delay);
            }
            match unless_closed(closing, self.attempt_after(wait)).await {
                None => return Recovery::Closed,
                Some(Ok(session)) => return Recovery::Restored(session),
                Some(Err(AttemptError::Stop(error))) => return Recovery::Failed(error),
                Some(Err(AttemptError::Retry(error))) => last = Some(error),
            }
        }
        Recovery::Failed(exhausted(policy.attempts.max(1), last.as_ref()))
    }

    async fn attempt_after(
        &self,
        wait: Option<Duration>,
    ) -> Result<Box<dyn Session>, AttemptError> {
        if let Some(wait) = wait {
            tokio::time::sleep(wait).await;
        }
        let mut session = (self.recipe.reconnect)().await.map_err(|error| {
            if retryable(&error) {
                AttemptError::Retry(error)
            } else {
                AttemptError::Stop(CoreError::from(&error))
            }
        })?;
        let limit = self.recipe.policy.probe_timeout;
        match tokio::time::timeout(limit, apply_setup(session.as_mut(), &self.recipe.setup)).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => return Err(AttemptError::Retry(error)),
            Err(_) => {
                return Err(AttemptError::Retry(DriverError::Network(
                    io::ErrorKind::TimedOut.into(),
                )));
            }
        }
        for (_, sql) in &self.replay {
            self.replay_one(session.as_mut(), sql).await?;
        }
        Ok(session)
    }

    async fn replay_one(&self, session: &mut dyn Session, sql: &str) -> Result<(), AttemptError> {
        let limit = self.recipe.policy.probe_timeout;
        match tokio::time::timeout(limit, session.execute(sql, &mut DiscardSink)).await {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(error)) if error.is_connection_lost() => Err(AttemptError::Retry(error)),
            Ok(Err(error)) => Err(AttemptError::Stop(CoreError {
                message: format!(
                    "The connection was re-established, but restoring `{sql}` failed, so the \
                     session stopped instead of running statements in the wrong place: {error}"
                ),
                ..CoreError::from(&error)
            })),
            Err(_) => Err(AttemptError::Retry(DriverError::Network(
                io::ErrorKind::TimedOut.into(),
            ))),
        }
    }
}

/// Network failures and servers that are starting, shutting down or full
/// (SQLSTATE classes 08, 53, 57; `MySQL` 1040 too many connections) may
/// succeed on a later attempt.
fn retryable(error: &DriverError) -> bool {
    match error {
        DriverError::Network(_) => true,
        DriverError::Server { code, .. } => {
            ["08", "53", "57"]
                .iter()
                .any(|class| code.starts_with(class))
                || code == "1040"
        }
        _ => false,
    }
}

fn exhausted(attempts: u32, last: Option<&DriverError>) -> CoreError {
    let reason = last.map_or_else(|| "no attempt was made".to_owned(), ToString::to_string);
    CoreError::new(
        ErrorKind::Network,
        format!(
            "The connection was lost and could not be re-established after {attempts} \
             attempts: {reason}"
        ),
    )
}

/// `None` when `closing` is cancelled before `work` finishes.
async fn unless_closed<T>(closing: &CancellationToken, work: impl Future<Output = T>) -> Option<T> {
    tokio::select! {
        biased;
        () = closing.cancelled() => None,
        value = work => Some(value),
    }
}

async fn sleep_until(deadline: Option<Instant>) {
    match deadline {
        Some(deadline) => tokio::time::sleep_until(deadline).await,
        None => std::future::pending().await,
    }
}

/// The error for a statement whose connection died while it ran: it may or
/// may not have been applied, and it is never run again automatically.
pub(crate) fn lost_in_flight(cause: &DriverError, in_transaction: bool) -> CoreError {
    let transaction = if in_transaction {
        " The open transaction was rolled back by the server, unless this statement was its \
         COMMIT."
    } else {
        ""
    };
    CoreError::new(
        ErrorKind::Network,
        format!(
            "Connection lost while the statement was running: it may or may not have been \
             applied, and it was not retried. Check the data before running it again.\
             {transaction} ({cause})"
        ),
    )
}

/// The session failure when an idle connection holding a transaction died.
pub(crate) fn transaction_lost(cause: &CoreError) -> CoreError {
    CoreError::new(
        ErrorKind::Network,
        format!(
            "The connection was lost while a transaction was open: the server rolled the \
             transaction back. Reconnect to continue. ({cause})"
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_transient_connect_failures_are_retried() {
        let server = |code: &str| DriverError::Server {
            code: code.to_owned(),
            message: String::new(),
        };
        assert!(retryable(&DriverError::Network(
            io::ErrorKind::ConnectionRefused.into()
        )));
        assert!(retryable(&server("57P03")));
        assert!(retryable(&server("53300")));
        assert!(retryable(&server("1040")));
        assert!(!retryable(&server("28P01")));
        assert!(!retryable(&DriverError::Auth("bad password".to_owned())));
        assert!(!retryable(&DriverError::Tls("expired".to_owned())));
    }

    #[test]
    fn messages_say_what_happened_to_the_data() {
        let cause = DriverError::Network(io::ErrorKind::UnexpectedEof.into());
        let lost = lost_in_flight(&cause, false);
        assert_eq!(lost.kind, ErrorKind::Network);
        assert!(lost.message.contains("may or may not have been applied"));
        assert!(!lost.message.contains("transaction"));
        assert!(lost_in_flight(&cause, true).message.contains("rolled back"));
        let failed = transaction_lost(&CoreError::from(&cause));
        assert!(failed.message.contains("rolled the transaction back"));
        let given_up = exhausted(5, Some(&cause));
        assert!(given_up.message.contains("after 5 attempts"));
    }
}

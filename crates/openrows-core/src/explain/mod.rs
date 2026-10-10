// SPDX-License-Identifier: Apache-2.0
//! Explain one statement on a session (FR-EDIT-07, docs/09 §6).
//!
//! [`admit`] gates the request first. [`explain`] then runs the engine's
//! explaining statements from [`openrows_explain::command`] on the session
//! actor, keeping their rows as text, and parses them into an analysed plan.
//! `PostgreSQL`/SQL Server `ANALYZE` runs between `BEGIN` and `ROLLBACK`, or inside a
//! savepoint when the user's session already has a transaction open (as its
//! driver reported after the last statement); the
//! rollback runs even when the statement failed or was cancelled, and a
//! failed rollback is reported as such.
mod admit;
mod rows;
mod session;

pub use admit::{Admission, admit, confirmation_token};

use crate::script::{ScriptCanceller, ScriptOutcome, follow};
use crate::{CoreError, ErrorKind, QueryEvent, SessionManager};
use openrows_explain::{
    AnalysedPlan, Engine, Format, Mode, ParseError, Wrap, analyse, command, parse, raw_text,
};
use openrows_sql::select::BoundSql;
use openrows_types::SessionId;
use rows::{TextRow, TextRows};
use session::ExplainSession;
use std::sync::Arc;

/// One statement to explain.
#[derive(Clone, Copy, Debug)]
pub struct ExplainRequest<'a> {
    /// The session's engine.
    pub engine: Engine,
    /// Plan only, or run and measure.
    pub mode: Mode,
    /// The statement, without a trailing `;`.
    pub statement: &'a str,
}

/// A parsed plan with the engine's own output.
#[derive(Clone, Debug)]
pub struct Explained {
    /// Flattened operators with measures and hotspots.
    pub plan: AnalysedPlan,
    /// The output as the engine's client shows it.
    pub raw: String,
    /// How `raw` is encoded.
    pub format: Format,
    /// The statement ran and its changes were rolled back.
    pub rolled_back: bool,
}

/// Why a statement could not be explained.
#[derive(Clone, Debug, thiserror::Error)]
pub enum ExplainError {
    /// The server rejected a statement, or the session failed.
    #[error("{0}")]
    Failed(CoreError),
    /// SQL Server session options could not be restored.
    #[error("Explain session options could not be restored: {0}")]
    NotRestored(CoreError),
    /// The user cancelled.
    #[error("Explain was cancelled")]
    Cancelled,
    /// The output was not in the expected format.
    #[error("The plan could not be read: {0}")]
    Unreadable(String),
    /// The rollback after `ANALYZE` failed: changes may persist.
    #[error("The changes made by Explain Analyze could not be rolled back: {0}")]
    NotRolledBack(CoreError),
}

impl From<ParseError> for ExplainError {
    fn from(error: ParseError) -> Self {
        Self::Unreadable(error.to_string())
    }
}

/// Explain `request` on `session`; `cancel` stops it (the rollback still runs).
///
/// # Errors
/// See [`ExplainError`].
pub async fn explain(
    manager: &SessionManager,
    session: SessionId,
    request: ExplainRequest<'_>,
    cancel: &ScriptCanceller,
) -> Result<Explained, ExplainError> {
    let session = ExplainSession::new(manager, session, request.engine == Engine::SqlServer)
        .map_err(ExplainError::Failed)?;
    if request.engine == Engine::SqlServer {
        // Drain preceding submissions so transaction state is current before wrapping.
        rows(&session, ("SELECT 1", false), cancel).await?;
    }
    let wrap = wrap_for(&session, request);
    let command = command(request.engine, request.mode, request.statement, wrap);
    let (output, needs_cleanup) = setup_and_run(&session, (&command, request.engine), cancel).await;
    // Cleanup has its own canceller, and attempts every step even if an option reset fails.
    if needs_cleanup {
        cleanup(&session, (&command, request.engine)).await?;
    }
    let output = output?;
    Ok(Explained {
        plan: analyse(parse(command.format, &output)?),
        raw: raw_text(command.format, &output),
        format: command.format,
        rolled_back: wrap != Wrap::None,
    })
}

/// `PostgreSQL`/SQL Server `ANALYZE` needs a transaction to roll back: a new one, or a
/// savepoint in the one the user has open (as the driver last reported).
fn wrap_for(session: &ExplainSession<'_>, request: ExplainRequest<'_>) -> Wrap {
    if !matches!(request.engine, Engine::Postgres | Engine::SqlServer)
        || request.mode != Mode::Analyze
    {
        Wrap::None
    } else if session.in_transaction() {
        Wrap::Savepoint
    } else {
        Wrap::Transaction
    }
}

/// Setup and run, keeping `PostgreSQL`'s failed-BEGIN guarantee.
async fn setup_and_run(
    session: &ExplainSession<'_>,
    (command, engine): (&openrows_explain::Command, Engine),
    cancel: &ScriptCanceller,
) -> (Result<Vec<TextRow>, ExplainError>, bool) {
    for (index, sql) in command.before.iter().enumerate() {
        if let Err(error) = rows(session, (sql, false), cancel).await {
            // A rejected first BEGIN/SAVEPOINT opened nothing. SQL Server option
            // setup still needs resetting if a prior setup statement succeeded.
            if index == 0 && matches!(error, ExplainError::Failed(_)) {
                return (Err(error), false);
            }
            return (Err(error), true);
        }
    }
    (
        rows(
            session,
            (&command.explain, engine == Engine::SqlServer),
            cancel,
        )
        .await,
        true,
    )
}

/// Always reset options before rollback, attempting rollback even after a reset error.
async fn cleanup(
    session: &ExplainSession<'_>,
    (command, engine): (&openrows_explain::Command, Engine),
) -> Result<(), ExplainError> {
    let cancel = ScriptCanceller::default();
    let mut failure = None;
    for sql in &command.after {
        if let Err(error) = rows(session, (sql, false), &cancel).await {
            let error = match error {
                ExplainError::Failed(core)
                    if engine == Engine::SqlServer && sql.starts_with("SET") =>
                {
                    ExplainError::NotRestored(core)
                }
                ExplainError::Failed(core) => ExplainError::NotRolledBack(core),
                other => other,
            };
            // A rollback failure is the more consequential failure.
            if failure.is_none() || matches!(error, ExplainError::NotRolledBack(_)) {
                failure = Some(error);
            }
        }
    }
    failure.map_or(Ok(()), Err)
}

/// Run `sql` and keep its rows as text.
async fn rows(
    session: &ExplainSession<'_>,
    (sql, showplan): (&str, bool),
    cancel: &ScriptCanceller,
) -> Result<Vec<TextRow>, ExplainError> {
    let sink = Arc::new(TextRows::new(showplan));
    let statement = BoundSql {
        sql: sql.to_owned(),
        params: Vec::new(),
    };
    let submitted = session.execute_into(statement, Arc::clone(&sink) as _);
    let mut failure = None;
    let outcome = follow(submitted, cancel, &mut |event| {
        if let QueryEvent::Failed { error } = event {
            failure = Some(error);
        }
    })
    .await;
    match outcome {
        ScriptOutcome::Completed if sink.over_budget() => Err(ExplainError::Unreadable(
            "plan output exceeds collection limits (8 MiB / 10000 rows)".to_owned(),
        )),
        ScriptOutcome::Completed => Ok(sink.take()),
        ScriptOutcome::Cancelled => Err(ExplainError::Cancelled),
        ScriptOutcome::Failed => Err(ExplainError::Failed(failure.unwrap_or_else(|| {
            CoreError::new(ErrorKind::Internal, "the statement failed without an error")
        }))),
    }
}

#[cfg(test)]
mod tests;

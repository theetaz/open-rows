// SPDX-License-Identifier: Apache-2.0
//! Explain one statement on a session (FR-EDIT-07, docs/09 §6).
//!
//! [`admit`] gates the request first. [`explain`] then runs the engine's
//! explaining statements from [`openrows_explain::command`] on the session
//! actor, keeping their rows as text, and parses them into an analysed plan.
//! `PostgreSQL` `ANALYZE` runs between `BEGIN` and `ROLLBACK`, or inside a
//! savepoint when the user's session already has a transaction open (as its
//! driver reported after the last statement); the
//! rollback runs even when the statement failed or was cancelled, and a
//! failed rollback is reported as such.
mod admit;
mod rows;

pub use admit::{Admission, admit, confirmation_token};

use crate::script::{ScriptCanceller, ScriptOutcome, follow};
use crate::{CoreError, ErrorKind, QueryEvent, SessionManager};
use openrows_explain::{
    AnalysedPlan, Engine, Format, Mode, ParseError, Wrap, analyse, command, parse, raw_text,
};
use openrows_sql::select::BoundSql;
use openrows_types::SessionId;
use rows::{TextRow, TextRows};
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
    let wrap = wrap_for(manager, session, request);
    let command = command(request.engine, request.mode, request.statement, wrap);
    let output = match run_all(manager, session, &command.before, cancel).await {
        Ok(()) => rows(manager, session, &command.explain, cancel).await,
        // A failed `BEGIN`/`SAVEPOINT` opened nothing to roll back…
        Err(ExplainError::Failed(error)) => return Err(ExplainError::Failed(error)),
        // …but a cancelled one may have.
        Err(other) => Err(other),
    };
    // The rollback must run even after a cancel, so it has its own canceller.
    let undo = ScriptCanceller::default();
    run_all(manager, session, &command.after, &undo)
        .await
        .map_err(|error| match error {
            ExplainError::Failed(error) => ExplainError::NotRolledBack(error),
            other => other,
        })?;
    let output = output?;
    Ok(Explained {
        plan: analyse(parse(command.format, &output)?),
        raw: raw_text(command.format, &output),
        format: command.format,
        rolled_back: !command.after.is_empty(),
    })
}

/// `PostgreSQL` `ANALYZE` needs a transaction to roll back: a new one, or a
/// savepoint in the one the user has open (as the driver last reported).
fn wrap_for(manager: &SessionManager, session: SessionId, request: ExplainRequest<'_>) -> Wrap {
    if request.engine != Engine::Postgres || request.mode != Mode::Analyze {
        Wrap::None
    } else if manager.in_transaction(session).unwrap_or(false) {
        Wrap::Savepoint
    } else {
        Wrap::Transaction
    }
}

/// Run `statements` in order, stopping at the first failure.
async fn run_all(
    manager: &SessionManager,
    session: SessionId,
    statements: &[String],
    cancel: &ScriptCanceller,
) -> Result<(), ExplainError> {
    for sql in statements {
        rows(manager, session, sql, cancel).await?;
    }
    Ok(())
}

/// Run `sql` and keep its rows as text.
async fn rows(
    manager: &SessionManager,
    session: SessionId,
    sql: &str,
    cancel: &ScriptCanceller,
) -> Result<Vec<TextRow>, ExplainError> {
    let sink = Arc::new(TextRows::default());
    let statement = BoundSql {
        sql: sql.to_owned(),
        params: Vec::new(),
    };
    let submitted = manager.execute_into(session, statement, Arc::clone(&sink) as _);
    let mut failure = None;
    let outcome = follow(submitted, cancel, &mut |event| {
        if let QueryEvent::Failed { error } = event {
            failure = Some(error);
        }
    })
    .await;
    match outcome {
        ScriptOutcome::Completed => Ok(sink.take()),
        ScriptOutcome::Cancelled => Err(ExplainError::Cancelled),
        ScriptOutcome::Failed => Err(ExplainError::Failed(failure.unwrap_or_else(|| {
            CoreError::new(ErrorKind::Internal, "the statement failed without an error")
        }))),
    }
}

#[cfg(test)]
mod tests;

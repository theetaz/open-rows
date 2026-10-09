// SPDX-License-Identifier: Apache-2.0
//! `query_explain` (FR-EDIT-07): explain or explain-analyze the statement at
//! the cursor or the selection on the tab's session, gated by
//! [`openrows_core::explain::admit`]; `query_cancel` with the request's
//! `explainId` cancels it (an analysed write is still rolled back).
use std::str::FromStr;

use openrows_core::explain::{self, Admission, ExplainError, ExplainRequest as Request};
use openrows_core::safety::SafetyPolicy;
use openrows_core::script::{self, Scope, ScriptCanceller};
use openrows_explain::{Engine, Mode};
use openrows_store::profile::Driver;
use openrows_types::SessionId;
use tauri::State;

use super::Queries;
use super::commands::policy;
use super::dto::byte_offset;
use super::explain_dto::{ExplainReply, ExplainRequest, PlanDto};
use crate::connections::{Connections, IpcError, IpcErrorKind};

/// The explain dialect of a profile's engine.
fn engine(driver: Driver) -> Result<Engine, IpcError> {
    match driver {
        Driver::Postgres => Ok(Engine::Postgres),
        Driver::Mysql => Ok(Engine::MySql),
        Driver::Mariadb => Ok(Engine::MariaDb),
        Driver::Sqlite => Ok(Engine::Sqlite),
        Driver::Sqlserver => Err(IpcError::new(
            IpcErrorKind::Unsupported,
            "Explain is not available for SQL Server yet",
        )),
    }
}

/// The one statement the request explains.
fn statement(request: &ExplainRequest, policy: SafetyPolicy) -> Result<String, IpcError> {
    let scope = match request.cursor {
        Some(cursor) => Scope::AtCursor(byte_offset(&request.sql, cursor as usize)),
        None => Scope::All,
    };
    let mut statements = script::statements(&request.sql, scope, policy.dialect);
    match statements.len() {
        0 => Err(IpcError::invalid("There is no SQL statement to explain")),
        1 => Ok(statements.remove(0)),
        _ => Err(IpcError::invalid("Select a single statement to explain")),
    }
}

/// The reply when the gate does not let the explain run yet (or ever).
fn refusal(
    admission: Admission,
    request: &ExplainRequest,
    (policy, statement): (SafetyPolicy, &str),
) -> Option<ExplainReply> {
    match admission {
        Admission::Run => None,
        Admission::Block(message) | Admission::Refuse(message) => {
            Some(ExplainReply::Blocked { message })
        }
        Admission::Confirm => {
            let token = explain::confirmation_token(policy, &request.session_id, statement);
            (request.confirm_token.as_deref() != Some(token.as_str())).then(|| {
                ExplainReply::ConfirmationRequired {
                    token,
                    statement: statement.to_owned(),
                }
            })
        }
    }
}

/// Explain the statement at the cursor, or the selection, on an open session.
///
/// Plans are always allowed. Explain Analyze of a possible write needs a
/// matching `confirmToken` (`confirmationRequired`), is refused on
/// read-only connections and on `MySQL`/`MariaDB` (`blocked`), and on
/// `PostgreSQL` runs in a transaction that is rolled back.
#[tauri::command]
#[specta::specta]
pub(crate) async fn query_explain(
    connections: State<'_, Connections>,
    queries: State<'_, Queries>,
    request: ExplainRequest,
) -> Result<ExplainReply, IpcError> {
    let session = SessionId::from_str(&request.session_id)
        .map_err(|_| IpcError::invalid("Invalid session id"))?;
    let owner = connections
        .owners
        .owner(session)
        .ok_or_else(|| IpcError::new(IpcErrorKind::NotFound, "The session is not open"))?;
    let profile = connections.profiles.get(owner.profile)?;
    let policy = policy(&profile);
    let statement = statement(&request, policy)?;
    let engine = engine(profile.driver)?;
    let mode = if request.analyze {
        Mode::Analyze
    } else {
        Mode::Plan
    };
    let admission = explain::admit(policy, engine, mode, &statement);
    if let Some(reply) = refusal(admission, &request, (policy, &statement)) {
        return Ok(reply);
    }
    let cancel = ScriptCanceller::default();
    queries.track(&request.explain_id, cancel.clone());
    let request_core = Request {
        engine,
        mode,
        statement: &statement,
    };
    let result = explain::explain(&connections.sessions, session, request_core, &cancel).await;
    queries.forget(&request.explain_id);
    match result {
        Ok(explained) => Ok(ExplainReply::Explained {
            plan: PlanDto::new(statement, explained),
        }),
        Err(ExplainError::Cancelled) => Ok(ExplainReply::Cancelled),
        Err(error) => Err(failure(error)),
    }
}

fn failure(error: ExplainError) -> IpcError {
    let message = error.to_string();
    match error {
        ExplainError::Failed(core) | ExplainError::NotRolledBack(core) => IpcError {
            message,
            ..IpcError::from(core)
        },
        ExplainError::Cancelled => IpcError::new(IpcErrorKind::Cancelled, message),
        ExplainError::Unreadable(_) => IpcError::new(IpcErrorKind::Protocol, message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openrows_sql::Dialect;

    fn request(sql: &str, cursor: Option<u32>) -> ExplainRequest {
        ExplainRequest {
            session_id: "s1".to_owned(),
            explain_id: "e1".to_owned(),
            sql: sql.to_owned(),
            cursor,
            analyze: true,
            confirm_token: None,
        }
    }

    const POLICY: SafetyPolicy = SafetyPolicy {
        read_only: false,
        confirm_writes: false,
        dialect: Dialect::Postgres,
    };

    #[test]
    fn the_cursor_picks_one_statement_and_a_selection_must_be_one() {
        let sql = "SELECT 1;\nDELETE FROM t;";
        assert_eq!(
            statement(&request(sql, Some(12)), POLICY).ok().as_deref(),
            Some("DELETE FROM t")
        );
        assert!(statement(&request(sql, None), POLICY).is_err());
        assert_eq!(
            statement(&request("SELECT 2;", None), POLICY)
                .ok()
                .as_deref(),
            Some("SELECT 2")
        );
        assert!(statement(&request("  -- nothing\n", None), POLICY).is_err());
    }

    #[test]
    fn a_confirmed_write_runs_only_with_its_token() -> Result<(), &'static str> {
        let statement = "DELETE FROM t";
        let asked = refusal(
            Admission::Confirm,
            &request(statement, None),
            (POLICY, statement),
        );
        let Some(ExplainReply::ConfirmationRequired { token, .. }) = asked else {
            return Err("expected a confirmation request");
        };
        let mut confirmed = request(statement, None);
        confirmed.confirm_token = Some(token);
        assert_eq!(
            refusal(Admission::Confirm, &confirmed, (POLICY, statement)),
            None
        );
        let blocked = refusal(
            Admission::Block("no".to_owned()),
            &confirmed,
            (POLICY, statement),
        );
        assert!(matches!(blocked, Some(ExplainReply::Blocked { .. })));
        Ok(())
    }
}

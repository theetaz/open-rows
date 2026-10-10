// SPDX-License-Identifier: Apache-2.0
//! `query_execute`, `query_cancel`, `results_release` and the `history_*` commands.
#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri deserialises command arguments into owned values"
)]
use std::str::FromStr;
use std::sync::Arc;
use std::time::Instant;

use openrows_core::safety::{Gate, SafetyPlan, SafetyPolicy};
use openrows_core::script::{Script, ScriptCanceller, run_script};
use openrows_results::ResultStore;
use openrows_sql::StatementKind;
use openrows_sql::select::BoundSql;
use openrows_store::LocalStore;
use openrows_store::profile::{ConnectionProfile, ProfileId};
use openrows_types::{QueryId, SessionId};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

use super::Queries;
use super::dto::{
    ExecuteReply, ExecuteRequest, HistoryItemDto, HistorySearchRequest, statement_dtos,
};
use super::events::{QueryEventDto, finished, translate};
use super::history::HistoryLog;
use super::params::{self, Binding};
use crate::connections::catalog_cache::after_statements;
use crate::connections::drivers::driver;
use crate::connections::{Connections, IpcError, IpcErrorKind};
use crate::exports::pins::ResultPins;

/// Everything a started run needs after the command returned.
struct Run {
    query_id: String,
    session: SessionId,
    profile: ConnectionProfile,
    /// The database the session runs on, recorded in history.
    database: String,
    /// Statement texts as written, recorded in history.
    statements: Vec<String>,
    /// What each statement does; DDL invalidates the schema cache.
    kinds: Vec<StatementKind>,
    /// The statements with their bound parameters, as executed.
    bound: Vec<BoundSql>,
    /// Rows kept per result set (the editor's row limit).
    max_rows: Option<usize>,
    stop_on_error: bool,
    cancel: ScriptCanceller,
    events: Channel<QueryEventDto>,
}

/// Split, bind, gate and start SQL on an open session.
///
/// Placeholders without values ask for them (`parametersRequired`);
/// read-only connections refuse writes (`blocked`); confirm-writes and
/// dangerous statements need a matching `confirmToken` (`confirmationRequired`).
/// Otherwise the run starts and events stream over `on_event`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn query_execute(
    app: AppHandle,
    connections: State<'_, Connections>,
    queries: State<'_, Queries>,
    request: ExecuteRequest,
    on_event: Channel<QueryEventDto>,
) -> Result<ExecuteReply, IpcError> {
    super::history_privacy::validate_reuse(&request.sql)?;
    let session = SessionId::from_str(&request.session_id)
        .map_err(|_| IpcError::invalid("Invalid session id"))?;
    let owner = connections
        .owners
        .owner(session)
        .ok_or_else(|| IpcError::new(IpcErrorKind::NotFound, "The session is not open"))?;
    let profile = connections.profiles.get(owner.profile)?;
    let policy = policy(&profile);
    let statements = request.statements(policy.dialect);
    if statements.is_empty() {
        return Err(IpcError::invalid("There is no SQL statement to run"));
    }
    let bound = match params::bind(&statements, policy.dialect, request.params.as_ref()) {
        Binding::Ready(bound) => bound,
        Binding::Ask(names) => return Ok(ExecuteReply::ParametersRequired { names }),
    };
    let plan = SafetyPlan::check(policy, statements);
    if let Some(reply) = refusal(&plan, &request) {
        return Ok(reply);
    }
    crate::security::authenticate_plan(&plan, &format!("{} / {}", profile.name, owner.database))
        .await?;
    app.state::<crate::security::Security>()
        .admit("query_execute")?;
    let run = Run {
        query_id: QueryId::new().to_string(),
        session,
        profile,
        database: owner.database,
        statements: plan.statements.iter().map(|s| s.sql.clone()).collect(),
        kinds: plan.statements.iter().map(|s| s.class.kind).collect(),
        bound,
        max_rows: request.row_cap(),
        stop_on_error: request.return_on_error,
        cancel: ScriptCanceller::default(),
        events: on_event,
    };
    connections
        .limits
        .apply_to_session(&connections.sessions, session, &run.profile)
        .map_err(IpcError::from)?;
    queries.track(&run.query_id, run.cancel.clone());
    let reply = ExecuteReply::Started {
        query_id: run.query_id.clone(),
        statements: statement_dtos(&plan),
    };
    tauri::async_runtime::spawn(execute(app, run));
    Ok(reply)
}

/// The profile's safety switches, judged in its engine's dialect.
pub(crate) fn policy(profile: &ConnectionProfile) -> SafetyPolicy {
    SafetyPolicy {
        mode: profile.safety,
        dialect: driver(profile.driver).dialect(),
    }
}

/// The reply when the gate does not let the run start yet (or ever).
fn refusal(plan: &SafetyPlan, request: &ExecuteRequest) -> Option<ExecuteReply> {
    let scope = request.session_id.as_str();
    match plan.gate() {
        Gate::Block => Some(ExecuteReply::Blocked {
            message: "This connection is read-only: nothing was run because a statement \
                      may modify the database."
                .to_owned(),
            statements: statement_dtos(plan),
        }),
        Gate::Confirm if !plan.allows(scope, request.confirm_token.as_deref()) => {
            Some(ExecuteReply::ConfirmationRequired {
                token: plan.confirmation_token(scope),
                statements: statement_dtos(plan),
            })
        }
        Gate::Run if asks_for_many(plan, request) => Some(ExecuteReply::ConfirmationRequired {
            token: plan.confirmation_token(scope),
            statements: statement_dtos(plan),
        }),
        Gate::Confirm | Gate::Run => None,
    }
}

/// Settings ▸ Execution ▸ Confirm multiple statements: a run of several
/// statements waits for the user once, unless this exact run was confirmed.
fn asks_for_many(plan: &SafetyPlan, request: &ExecuteRequest) -> bool {
    let scope = request.session_id.as_str();
    request.confirm_multiple
        && plan.statements.len() > 1
        && request.confirm_token.as_deref() != Some(plan.confirmation_token(scope).as_str())
}

/// Run the statements in order, stream events and record history.
async fn execute(app: AppHandle, run: Run) {
    let connections = app.state::<Connections>();
    let store = app.state::<Arc<ResultStore>>();
    let local = app.state::<LocalStore>();
    let mut history = HistoryLog::new(
        run.profile.record_history,
        run.profile.id.to_string(),
        Some(run.database.clone()),
        run.statements.clone(),
    );
    let started = Instant::now();
    let outcome = run_script(
        &connections.sessions,
        run.session,
        Script {
            statements: &run.bound,
            max_rows: run.max_rows,
            stop_on_error: run.stop_on_error,
            sink: None,
        },
        &run.cancel,
        |event| {
            if let Some(record) = history.observe(&event) {
                // History is best effort: a failed write must not fail the query.
                super::history_privacy::record(
                    &local,
                    record,
                    (
                        super::history_privacy::redacts(&run.profile),
                        policy(&run.profile).dialect,
                    ),
                );
            }
            // A closed channel means the tab is gone; the run still finishes.
            let _ = run.events.send(translate(event, &store));
        },
    )
    .await;
    // Even a failed or cancelled script may have run some of its DDL.
    let profile_id = run.profile.id.to_string();
    after_statements(
        &app,
        (&profile_id, &run.database),
        run.kinds.iter().copied(),
    );
    let _ = run.events.send(finished(outcome, started.elapsed()));
    app.state::<Queries>().forget(&run.query_id);
}

/// Cancel a running query and every statement after it; unknown ids are ignored.
#[tauri::command]
#[specta::specta]
pub(crate) fn query_cancel(queries: State<'_, Queries>, query_id: String) {
    queries.cancel(&query_id);
}

/// Drop results whose tab closed or was re-run; one a running export still
/// reads is dropped when that export ends.
#[tauri::command]
#[specta::specta]
pub(crate) fn results_release(
    store: State<'_, Arc<ResultStore>>,
    pins: State<'_, ResultPins>,
    result_ids: Vec<String>,
) {
    for id in &result_ids {
        if pins.release(id) {
            store.remove(id);
        }
    }
}

/// Entries of the local history, newest first: one connection or all of them,
/// narrowed to statements matching the free text.
#[tauri::command]
#[specta::specta]
pub(crate) fn history_search(
    local: State<'_, LocalStore>,
    request: HistorySearchRequest,
) -> Result<Vec<HistoryItemDto>, IpcError> {
    let entries = local
        .search_history(
            request.profile_id.as_deref(),
            &request.text,
            request.limit.min(500),
        )
        .map_err(storage_error)?;
    Ok(entries.into_iter().map(HistoryItemDto::from).collect())
}

/// Delete one history entry; `false` when it was already gone.
#[tauri::command]
#[specta::specta]
pub(crate) fn history_delete(local: State<'_, LocalStore>, id: f64) -> Result<bool, IpcError> {
    let id = row_id(id).ok_or_else(|| IpcError::invalid("Invalid history entry"))?;
    local.delete_history(id).map_err(storage_error)
}

/// Delete every history entry of one saved connection.
#[tauri::command]
#[specta::specta]
pub(crate) fn history_clear(
    local: State<'_, LocalStore>,
    profile_id: Option<String>,
) -> Result<(), IpcError> {
    let Some(profile_id) = profile_id else {
        return local.clear_all_history().map(drop).map_err(storage_error);
    };
    let id = ProfileId::from_str(&profile_id)?;
    local
        .clear_history(&id.to_string())
        .map(drop)
        .map_err(storage_error)
}

fn storage_error(error: openrows_store::StoreError) -> IpcError {
    IpcError::new(IpcErrorKind::Storage, error.to_string())
}

/// History ids cross the IPC boundary as JS numbers, which are exact up to 2^53.
#[expect(
    clippy::cast_possible_truncation,
    reason = "checked: the value is a whole number inside the exact JS integer range"
)]
fn row_id(id: f64) -> Option<i64> {
    const EXACT: f64 = 9_007_199_254_740_992.0;
    (id.is_finite() && (1.0..=EXACT).contains(&id) && id.fract() == 0.0).then_some(id as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::queries::dto::RunMode;

    fn run(sql: &str, confirm_multiple: bool, token: Option<String>) -> ExecuteRequest {
        ExecuteRequest {
            session_id: "s".to_owned(),
            sql: sql.to_owned(),
            mode: RunMode::All,
            cursor: None,
            confirm_token: token,
            max_rows: None,
            return_on_error: true,
            params: None,
            auto_limit: None,
            confirm_multiple,
        }
    }

    #[test]
    fn execute_entry_uses_policy_confirmation_before_authentication() {
        use openrows_types::safety::Safety;
        for mode in [Safety::ConfirmAll, Safety::AuthenticateAll] {
            let policy = SafetyPolicy {
                mode,
                ..SafetyPolicy::default()
            };
            let request = run("SELECT 1", false, None);
            let plan = SafetyPlan::check(policy, request.statements(policy.dialect));
            let token = match refusal(&plan, &request) {
                Some(ExecuteReply::ConfirmationRequired { token, .. }) => Some(token),
                _ => None,
            };
            assert!(token.is_some(), "read was not reviewed");
            assert!(refusal(&plan, &run("SELECT 1", false, token)).is_none());
        }
    }

    #[test]
    fn several_statements_wait_for_confirmation_once_when_asked() {
        let policy = SafetyPolicy::default();
        let many = run("SELECT 1; SELECT 2", true, None);
        let plan = SafetyPlan::check(policy, many.statements(policy.dialect));
        let token = match refusal(&plan, &many) {
            Some(ExecuteReply::ConfirmationRequired { token, .. }) => Some(token),
            _ => None,
        };
        assert!(token.is_some(), "expected a confirmation");
        assert!(refusal(&plan, &run("SELECT 1; SELECT 2", true, token)).is_none());
        assert!(refusal(&plan, &run("SELECT 1; SELECT 2", false, None)).is_none());
        let one = run("SELECT 1", true, None);
        let single = SafetyPlan::check(policy, one.statements(policy.dialect));
        assert!(refusal(&single, &one).is_none());
    }
}

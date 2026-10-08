// SPDX-License-Identifier: Apache-2.0
//! `query_execute`, `query_cancel`, `results_release` and `history_recent`.
#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri deserialises command arguments into owned values"
)]
use std::str::FromStr;
use std::sync::Arc;
use std::time::Instant;

use openrows_core::safety::{Gate, SafetyPlan, SafetyPolicy};
use openrows_core::script::{ScriptCanceller, run_script};
use openrows_results::ResultStore;
use openrows_store::LocalStore;
use openrows_store::profile::{ConnectionProfile, ProfileId};
use openrows_types::{QueryId, SessionId};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

use super::Queries;
use super::dto::{ExecuteReply, ExecuteRequest, HistoryItemDto, statement_dtos};
use super::events::{QueryEventDto, finished, translate};
use super::history::HistoryLog;
use crate::connections::drivers::driver;
use crate::connections::{Connections, IpcError, IpcErrorKind};

/// Everything a started run needs after the command returned.
struct Run {
    query_id: String,
    session: SessionId,
    profile: ConnectionProfile,
    /// The database the session runs on, recorded in history.
    database: String,
    statements: Vec<String>,
    cancel: ScriptCanceller,
    events: Channel<QueryEventDto>,
}

/// Split, gate and start SQL on an open session.
///
/// Read-only connections refuse writes (`blocked`); confirm-writes and
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
    let plan = SafetyPlan::check(policy, statements);
    if let Some(reply) = refusal(&plan, &request) {
        return Ok(reply);
    }
    let run = Run {
        query_id: QueryId::new().to_string(),
        session,
        profile,
        database: owner.database,
        statements: plan.statements.iter().map(|s| s.sql.clone()).collect(),
        cancel: ScriptCanceller::default(),
        events: on_event,
    };
    queries.track(&run.query_id, run.cancel.clone());
    let reply = ExecuteReply::Started {
        query_id: run.query_id.clone(),
        statements: statement_dtos(&plan),
    };
    tauri::async_runtime::spawn(execute(app, run));
    Ok(reply)
}

/// The profile's safety switches, judged in its engine's dialect.
fn policy(profile: &ConnectionProfile) -> SafetyPolicy {
    SafetyPolicy {
        read_only: profile.safety.read_only,
        confirm_writes: profile.safety.confirm_writes,
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
        Gate::Confirm | Gate::Run => None,
    }
}

/// Run the statements in order, stream events and record history.
async fn execute(app: AppHandle, run: Run) {
    let connections = app.state::<Connections>();
    let store = app.state::<Arc<ResultStore>>();
    let local = app.state::<LocalStore>();
    let mut history = HistoryLog::new(
        run.profile.id.to_string(),
        Some(run.database.clone()),
        run.statements.clone(),
    );
    let started = Instant::now();
    let outcome = run_script(
        &connections.sessions,
        run.session,
        &run.statements,
        &run.cancel,
        |event| {
            if let Some(record) = history.observe(&event) {
                // History is best effort: a failed write must not fail the query.
                let _ = local.record_history(&record);
            }
            // A closed channel means the tab is gone; the run still finishes.
            let _ = run.events.send(translate(event, &store));
        },
    )
    .await;
    let _ = run.events.send(finished(outcome, started.elapsed()));
    app.state::<Queries>().forget(&run.query_id);
}

/// Cancel a running query and every statement after it; unknown ids are ignored.
#[tauri::command]
#[specta::specta]
pub(crate) fn query_cancel(queries: State<'_, Queries>, query_id: String) {
    queries.cancel(&query_id);
}

/// Drop results whose tab closed or was re-run.
#[tauri::command]
#[specta::specta]
pub(crate) fn results_release(store: State<'_, Arc<ResultStore>>, result_ids: Vec<String>) {
    for id in &result_ids {
        store.remove(id);
    }
}

/// The newest statements run on a saved connection, newest first.
#[tauri::command]
#[specta::specta]
pub(crate) fn history_recent(
    local: State<'_, LocalStore>,
    profile_id: String,
    limit: u32,
) -> Result<Vec<HistoryItemDto>, IpcError> {
    let id = ProfileId::from_str(&profile_id)?;
    let entries = local
        .recent_history(&id.to_string(), limit.min(500))
        .map_err(|error| IpcError::new(IpcErrorKind::Storage, error.to_string()))?;
    Ok(entries.into_iter().map(HistoryItemDto::from).collect())
}

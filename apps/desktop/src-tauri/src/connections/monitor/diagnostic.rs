// SPDX-License-Identifier: Apache-2.0
//! Explicit bounded `PostgreSQL` diagnostics use a disposable session, never user transactions.
use super::{
    MonitorTarget,
    advisor::{failure, now_ms},
    diagnostic_gate, reader,
};
use crate::{
    connections::{Connections, IpcError, IpcErrorKind},
    queries::{
        Queries,
        explain_dto::{ExplainReply, PlanDto},
    },
};
use openrows_core::{
    explain::{self, ExplainRequest},
    script::{ScriptCanceller, ScriptOutcome},
};
use openrows_driver_api::Session;
use openrows_explain::{Engine, Mode};
use openrows_monitor::Monitor;
use openrows_types::SessionId;
use serde::Deserialize;
use specta::Type;
use std::{sync::Arc, time::Duration};
use tauri::{Manager, State};

/// Complete user-reviewed SQL and explicit diagnostic context.
#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MonitorExplain {
    /// Profile/database scope.
    pub(super) target: MonitorTarget,
    /// Native sampled statement identity.
    pub(super) statement_id: String,
    /// Cancellation identity.
    pub(super) explain_id: String,
    /// Complete user-reviewed SQL.
    pub(super) sql: String,
    /// Exact reviewed search-path schema.
    pub(super) schema: String,
    /// Explicit measured-execution choice.
    pub(super) analyze: bool,
    /// SQL/context/policy-bound native consent.
    pub(super) confirm_token: Option<String>,
}

/// Plan-only by default in the UI; every measured execution confirms natively.
#[tauri::command]
#[specta::specta]
pub(crate) async fn monitor_explain(
    state: State<'_, Connections>,
    queries: State<'_, Queries>,
    request: MonitorExplain,
) -> Result<ExplainReply, IpcError> {
    state
        .app
        .state::<crate::security::Security>()
        .admit("monitor_explain")?;
    let monitor = state
        .monitors
        .get(&request.target.key())
        .map_err(|error| failure(&error))?;
    let _reservation = monitor
        .begin_diagnostic()
        .map_err(|error| failure(&error))?;
    let profile = state.profiles.get(super::super::profiles::profile_id(
        &request.target.profile_id,
    )?)?;
    reader::ensure_postgres(profile.driver)?;
    let sql = diagnostic_gate::statement(&request)?;
    let expected = monitor
        .investigate(&request.statement_id)
        .map_err(|error| failure(&error))?
        .query;
    if expected.is_none() || openrows_sql::normalize_monitor_query(&sql) != expected {
        return Err(IpcError::invalid(
            "Reviewed SQL differs from this sampled statement; use an ordinary editor tab for another query",
        ));
    }
    if let Some(reply) = diagnostic_gate::gate(&request, &profile, &monitor.generation(), &sql) {
        if let ExplainReply::ConfirmationRequired { token, statement } = reply {
            if let Some(token) = monitor
                .confirm_diagnostic(&token, request.confirm_token.as_deref())
                .map_err(|error| failure(&error))?
            {
                return Ok(ExplainReply::ConfirmationRequired { token, statement });
            }
        } else {
            return Ok(reply);
        }
    }
    let cancel = ScriptCanceller::default();
    queries.track(&request.explain_id, cancel.clone());
    let result = authorized_diagnostic(&state, &monitor, (&request, &sql), &cancel).await;
    queries.forget(&request.explain_id);
    result
}

async fn authorized_diagnostic(
    state: &Connections,
    monitor: &Arc<Monitor>,
    (request, sql): (&MonitorExplain, &str),
    cancel: &ScriptCanceller,
) -> Result<ExplainReply, IpcError> {
    let profile = state.profiles.get(super::super::profiles::profile_id(
        &request.target.profile_id,
    )?)?;
    let checked = if request.analyze {
        sql.to_owned()
    } else {
        format!("EXPLAIN {sql}")
    };
    crate::security::authorize(&profile, vec![checked], &request.target.database).await?;
    state
        .app
        .state::<crate::security::Security>()
        .admit("monitor_explain")?;
    if cancel.is_cancelled() || *monitor.stopped_signal().borrow() {
        return Ok(ExplainReply::Cancelled);
    }
    diagnostic(state, monitor, (request, sql), cancel).await
}

async fn diagnostic(
    state: &Connections,
    monitor: &Arc<Monitor>,
    (request, sql): (&MonitorExplain, &str),
    cancel: &ScriptCanceller,
) -> Result<ExplainReply, IpcError> {
    let mut stopped = monitor.stopped_signal();
    if *stopped.borrow() {
        return Ok(ExplainReply::Cancelled);
    }
    let opened = tokio::select! {
        result = tokio::time::timeout(Duration::from_secs(16), reader::open(state, &request.target)) => result.map_err(|_| timeout())??,
        _ = stopped.changed() => return Ok(ExplainReply::Cancelled),
    };
    let opened = super::diagnostic_session::DiagnosticSession(opened.0 as Box<dyn Session>);
    if *stopped.borrow() || cancel.is_cancelled() {
        Box::new(opened).close().await.map_err(|_| {
            IpcError::new(IpcErrorKind::Internal, "Diagnostic session cleanup failed")
        })?;
        return Ok(ExplainReply::Cancelled);
    }
    let session = state
        .sessions
        .open(async move { Ok(Box::new(opened) as Box<dyn Session>) })
        .await?;
    let result = tokio::select! {
        result = tokio::time::timeout(Duration::from_secs(20), run(state, monitor, session, (request, sql, cancel))) => result.unwrap_or_else(|_| Err(timeout())),
        _ = stopped.changed() => Ok(ExplainReply::Cancelled),
    };
    cancel.cancel();
    // Closing the disposable session rolls back even when timeout/lock dropped Explain's future.
    let closed = state.sessions.close(session).await;
    closed
        .map_err(|_| IpcError::new(IpcErrorKind::Internal, "Diagnostic session cleanup failed"))?;
    result
}
async fn run(
    state: &Connections,
    monitor: &Monitor,
    session: SessionId,
    (request, sql, cancel): (&MonitorExplain, &str, &ScriptCanceller),
) -> Result<ExplainReply, IpcError> {
    setup(state, session, request, cancel).await?;
    let request_core = ExplainRequest {
        engine: Engine::Postgres,
        mode: if request.analyze {
            Mode::Analyze
        } else {
            Mode::Plan
        },
        statement: sql,
    };
    state
        .app
        .state::<crate::security::Security>()
        .admit("monitor_explain")?;
    if *monitor.stopped_signal().borrow() {
        return Ok(ExplainReply::Cancelled);
    }
    let explained = explain::explain(&state.sessions, session, request_core, cancel)
        .await
        .map_err(|_| {
            IpcError::invalid("Diagnostic failed; review statement, grants and context")
        })?;
    state
        .app
        .state::<crate::security::Security>()
        .admit("monitor_explain")?;
    let input = monitor
        .advisor_input(now_ms())
        .map_err(|error| failure(&error))?;
    let tables = if input
        .tables_coverage
        .fresh(now_ms(), openrows_monitor::advisor::CATALOG_MAX_AGE_MS)
    {
        input.tables
    } else {
        Vec::new()
    };
    monitor
        .record_plan(openrows_monitor::advisor::project_plan(
            &explained.plan,
            (&request.statement_id, now_ms()),
            &tables,
        ))
        .map_err(|error| failure(&error))?;
    Ok(ExplainReply::Explained {
        plan: PlanDto::monitor(sql, explained),
    })
}
async fn setup(
    state: &Connections,
    session: SessionId,
    request: &MonitorExplain,
    cancel: &ScriptCanceller,
) -> Result<(), IpcError> {
    let profile = state.profiles.get(super::super::profiles::profile_id(
        &request.target.profile_id,
    )?)?;
    for setting in diagnostic_gate::settings(&request.schema, profile.safety.read_only()) {
        let statement = openrows_sql::select::BoundSql {
            sql: setting,
            params: Vec::new(),
        };
        if openrows_core::script::run_bound(&state.sessions, session, statement, cancel, |_| {})
            .await
            != ScriptOutcome::Completed
        {
            return Err(IpcError::invalid("Diagnostic context setup failed"));
        }
    }
    Ok(())
}

fn timeout() -> IpcError {
    IpcError::new(
        IpcErrorKind::Timeout,
        "Diagnostic deadline exceeded; session closed",
    )
}

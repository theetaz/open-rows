// SPDX-License-Identifier: Apache-2.0
//! Explicit `HypoPG` what-if plans use a disposable session and estimated costs only.
use super::{
    MonitorTarget,
    advisor::{failure, now_ms},
    diagnostic_gate,
    diagnostic_session::DiagnosticSession,
    reader,
};
use crate::{
    connections::{Connections, IpcError, IpcErrorKind},
    queries::explain_dto::ExplainReply,
};
use arrow_array::{Array, RecordBatch, StringArray};
use arrow_schema::SchemaRef;
use openrows_driver_api::{ResultSink, Session, SinkFlow};
use openrows_monitor::Monitor;
use openrows_store::profile::ConnectionProfile;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{sync::Arc, time::Duration};
use tauri::{Manager, State};

/// One explicit, connection-scoped hypothetical-index check.
#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HypoPgRequest {
    target: MonitorTarget,
    statement_id: String,
    suggestion_id: String,
    sql: String,
    schema: String,
    enabled: bool,
    confirm_token: Option<String>,
}

/// Estimated plan costs are comparable within this session, not runtime savings.
#[derive(Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HypoPgEstimate {
    baseline_cost: f64,
    hypothetical_cost: f64,
}

/// Native safety admission is shared with reviewed plan-only Explain.
#[derive(Serialize, Type)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(crate) enum HypoPgReply {
    /// Current policy requires native confirmation before any diagnostic work.
    ConfirmationRequired { token: String, statement: String },
    /// Two estimates from the same disposable session.
    Estimate { estimate: HypoPgEstimate },
}

/// Validate one fresh index candidate, without installing an extension or executing the proposal.
#[tauri::command]
#[specta::specta]
pub(crate) async fn monitor_hypopg(
    state: State<'_, Connections>,
    request: HypoPgRequest,
) -> Result<HypoPgReply, IpcError> {
    state
        .app
        .state::<crate::security::Security>()
        .admit("monitor_hypopg")?;
    if !request.enabled {
        return Err(IpcError::invalid("Enable HypoPG for this explicit check"));
    }
    let monitor = state
        .monitors
        .get(&request.target.key())
        .map_err(|e| failure(&e))?;
    let _reservation = monitor.begin_diagnostic().map_err(|e| failure(&e))?;
    let profile = state.profiles.get(super::super::profiles::profile_id(
        &request.target.profile_id,
    )?)?;
    reader::ensure_engine(profile.driver)?;
    let (sql, reply) = reviewed_sql(&monitor, &profile, &request)?;
    if let Some(reply) = reply {
        return Ok(reply);
    }
    let card = monitor
        .advice(now_ms())
        .map_err(|e| failure(&e))?
        .into_iter()
        .find(|card| {
            card.id == request.suggestion_id
                && card.statement_id.as_deref() == Some(&request.statement_id)
        })
        .ok_or_else(|| IpcError::invalid("Index evidence changed or expired"))?;
    let index_sql = candidate_sql(&card)?;
    crate::security::authorize(
        &profile,
        vec![format!("EXPLAIN {sql}")],
        &request.target.database,
    )
    .await?;
    state
        .app
        .state::<crate::security::Security>()
        .admit("monitor_hypopg")?;
    let still_current = monitor
        .advice(now_ms())
        .map_err(|e| failure(&e))?
        .iter()
        .any(|latest| latest.id == card.id && latest.sql == card.sql);
    if !still_current {
        return Err(IpcError::invalid("Index evidence changed or expired"));
    }
    let estimate = check(&state, &monitor, (&request, &sql, &index_sql)).await?;
    Ok(HypoPgReply::Estimate { estimate })
}

fn reviewed_sql(
    monitor: &Monitor,
    profile: &ConnectionProfile,
    request: &HypoPgRequest,
) -> Result<(String, Option<HypoPgReply>), IpcError> {
    let checked = super::diagnostic::MonitorExplain {
        target: request.target.clone(),
        statement_id: request.statement_id.clone(),
        explain_id: "hypopg".into(),
        sql: request.sql.clone(),
        schema: request.schema.clone(),
        analyze: false,
        confirm_token: request.confirm_token.clone(),
    };
    let sql = diagnostic_gate::statement(&checked)?;
    let expected = monitor
        .investigate(&request.statement_id)
        .map_err(|e| failure(&e))?
        .query;
    if expected.is_none() || openrows_sql::normalize_monitor_query(&sql) != expected {
        return Err(IpcError::invalid(
            "Reviewed SQL differs from the sampled statement",
        ));
    }
    let reply = match diagnostic_gate::gate(&checked, profile, &monitor.generation(), &sql) {
        Some(ExplainReply::Blocked { message }) => return Err(IpcError::invalid(message)),
        Some(ExplainReply::ConfirmationRequired { token, statement }) => monitor
            .confirm_diagnostic(&token, request.confirm_token.as_deref())
            .map_err(|e| failure(&e))?
            .map(|_| HypoPgReply::ConfirmationRequired { token, statement }),
        Some(_) => return Err(IpcError::invalid("Diagnostic admission failed")),
        None => None,
    };
    Ok((sql, reply))
}

fn candidate_sql(card: &openrows_monitor::advisor::Suggestion) -> Result<String, IpcError> {
    if !matches!(card.rule_id.as_str(), "pg.selective-scan" | "pg.join-key") {
        return Err(IpcError::invalid("HypoPG supports only index candidates"));
    }
    let sql = card
        .sql
        .as_deref()
        .ok_or_else(|| IpcError::invalid("No index proposal"))?;
    let sql = sql
        .strip_prefix("CREATE INDEX CONCURRENTLY ON ")
        .ok_or_else(|| IpcError::invalid("Unsupported index proposal"))?;
    Ok(format!("CREATE INDEX ON {}", sql.trim_end_matches(';')))
}

async fn check(
    state: &Connections,
    monitor: &Arc<Monitor>,
    (request, sql, index_sql): (&HypoPgRequest, &str, &str),
) -> Result<HypoPgEstimate, IpcError> {
    let mut stopped = monitor.stopped_signal();
    if *stopped.borrow() {
        return Err(IpcError::invalid("Monitor stopped"));
    }
    let opened = tokio::time::timeout(
        Duration::from_secs(16),
        reader::open(state, &request.target),
    )
    .await
    .map_err(|_| timeout())??;
    let mut session = DiagnosticSession(opened.0 as Box<dyn Session>);
    let result = tokio::select! {
        result = tokio::time::timeout(Duration::from_secs(20), run(&mut session, request, sql, index_sql)) => result.unwrap_or_else(|_| Err(timeout())),
        _ = stopped.changed() => Err(IpcError::invalid("Monitor stopped")),
    };
    let closed = Box::new(session).close().await;
    if closed.is_err() {
        return Err(IpcError::new(
            IpcErrorKind::Internal,
            "HypoPG session cleanup failed",
        ));
    }
    result
}

async fn run(
    session: &mut DiagnosticSession,
    request: &HypoPgRequest,
    sql: &str,
    index_sql: &str,
) -> Result<HypoPgEstimate, IpcError> {
    for setting in diagnostic_gate::settings(&request.schema, false) {
        execute(session, &setting).await?;
    }
    let namespace = query_text(session, "SELECT n.nspname FROM pg_extension e JOIN pg_namespace n ON n.oid = e.extnamespace WHERE e.extname = 'hypopg'").await?
        .ok_or_else(|| IpcError::new(IpcErrorKind::Unsupported, "HypoPG is not installed in this database"))?;
    let function = format!("\"{}\"", namespace.replace('"', "\"\""));
    let baseline = plan_cost(session, sql).await?;
    let create = format!(
        "SELECT * FROM {function}.hypopg_create_index('{}')",
        index_sql.replace('\'', "''")
    );
    let result = async {
        execute(session, &create).await?;
        plan_cost(session, sql).await
    }
    .await;
    let reset = execute(session, &format!("SELECT {function}.hypopg_reset()")).await;
    let hypothetical_cost = result?;
    reset?;
    Ok(HypoPgEstimate {
        baseline_cost: baseline,
        hypothetical_cost,
    })
}

async fn plan_cost(session: &mut DiagnosticSession, sql: &str) -> Result<f64, IpcError> {
    let text = query_text(session, &format!("EXPLAIN (FORMAT JSON) {sql}"))
        .await?
        .ok_or_else(|| IpcError::invalid("Plan cost unavailable"))?;
    let json: serde_json::Value =
        serde_json::from_str(&text).map_err(|_| IpcError::invalid("Plan cost unavailable"))?;
    json.get(0)
        .and_then(|root| root.get("Plan"))
        .and_then(|plan| plan.get("Total Cost"))
        .and_then(serde_json::Value::as_f64)
        .filter(|cost| cost.is_finite() && *cost >= 0.0)
        .ok_or_else(|| IpcError::invalid("Plan cost unavailable"))
}

async fn execute(session: &mut DiagnosticSession, sql: &str) -> Result<(), IpcError> {
    let mut sink = OneText::default();
    session.execute(sql, &mut sink).await.map_err(|_| {
        IpcError::invalid("HypoPG diagnostic denied or failed; check grants and context")
    })?;
    Ok(())
}

async fn query_text(
    session: &mut DiagnosticSession,
    sql: &str,
) -> Result<Option<String>, IpcError> {
    let mut sink = OneText::default();
    session.execute(sql, &mut sink).await.map_err(|_| {
        IpcError::invalid("HypoPG diagnostic denied or failed; check grants and context")
    })?;
    Ok(sink.0)
}

#[derive(Default)]
struct OneText(Option<String>);
impl ResultSink for OneText {
    fn begin_result(&mut self, _schema: SchemaRef) {}
    fn push_batch(&mut self, batch: RecordBatch) -> SinkFlow {
        if self.0.is_none() {
            self.0 = batch
                .column(0)
                .as_any()
                .downcast_ref::<StringArray>()
                .and_then(|column| {
                    (!column.is_empty() && !column.is_null(0)).then(|| column.value(0).to_owned())
                });
        }
        SinkFlow::Stop
    }
}

fn timeout() -> IpcError {
    IpcError::new(IpcErrorKind::Timeout, "HypoPG diagnostic deadline exceeded")
}

#[cfg(test)]
#[path = "hypopg_tests.rs"]
mod tests;

// SPDX-License-Identifier: Apache-2.0
//! Deterministic advisor IPC and editor-only proposal admission.
use super::MonitorTarget;
use crate::connections::{Connections, IpcError, IpcErrorKind};
use openrows_monitor::{Investigation, advisor::Suggestion};
use tauri::{Manager, State};

/// Current wall-clock milliseconds for native evidence freshness checks.
pub(super) fn now_ms() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |duration| duration.as_secs_f64() * 1_000.0)
}
/// Preserve existing monitor error categories at the IPC boundary.
pub(super) fn failure(error: &openrows_monitor::MonitorError) -> IpcError {
    let kind = match error {
        openrows_monitor::MonitorError::Stopped => IpcErrorKind::NotFound,
        openrows_monitor::MonitorError::Limit => IpcErrorKind::Invalid,
        openrows_monitor::MonitorError::Encoding => IpcErrorKind::Internal,
        openrows_monitor::MonitorError::Unsupported => IpcErrorKind::Unsupported,
    };
    IpcError::new(kind, error.to_string())
}
/// Read a live investigation; source SQL has already passed core redaction.
#[tauri::command]
#[specta::specta]
pub(crate) async fn monitor_investigate(
    state: State<'_, Connections>,
    target: MonitorTarget,
    statement_id: String,
) -> Result<Investigation, IpcError> {
    state
        .app
        .state::<crate::security::Security>()
        .admit("monitor_investigate")?;
    if statement_id.len() > 256 {
        return Err(IpcError::invalid("Invalid statement identity"));
    }
    state
        .monitors
        .get(&target.key())
        .map_err(|error| failure(&error))?
        .investigate(&statement_id)
        .map_err(|error| failure(&error))
}
/// Evaluate current native evidence; deterministic advice needs no AI/provider configuration.
#[tauri::command]
#[specta::specta]
pub(crate) async fn monitor_advice(
    state: State<'_, Connections>,
    target: MonitorTarget,
) -> Result<Vec<Suggestion>, IpcError> {
    state
        .app
        .state::<crate::security::Security>()
        .admit("monitor_advice")?;
    state
        .monitors
        .get(&target.key())
        .map_err(|error| failure(&error))?
        .advice(now_ms())
        .map_err(|error| failure(&error))
}
/// Revalidate a suggestion then enqueue exact SQL for editor review; never execute it.
#[tauri::command]
#[specta::specta]
pub(crate) async fn monitor_propose(
    state: State<'_, Connections>,
    target: MonitorTarget,
    suggestion_id: String,
) -> Result<(), IpcError> {
    state
        .app
        .state::<crate::security::Security>()
        .admit("monitor_propose")?;
    let monitor = state
        .monitors
        .get(&target.key())
        .map_err(|error| failure(&error))?;
    let card = monitor
        .advice(now_ms())
        .map_err(|error| failure(&error))?
        .into_iter()
        .find(|card| card.id == suggestion_id)
        .ok_or_else(|| IpcError::invalid("Evidence changed or expired; refresh the advisor"))?;
    let sql = card
        .sql
        .ok_or_else(|| IpcError::invalid("This suggestion has no SQL proposal"))?;
    let input = openrows_mcp::ToolInput {
        connection: Some(target.profile_id),
        database: Some(target.database),
        sql: Some(format!(
            "-- {} v{} · {} confidence\n-- {}\n-- {}\n{sql}",
            card.rule_id,
            card.version,
            card.confidence,
            card.evidence.replace(['\r', '\n'], " "),
            card.trade_offs.replace(['\r', '\n'], " ")
        )),
        ..Default::default()
    };
    crate::mcp::proposals::enqueue(&state.app, &input)
        .map_err(|_| IpcError::invalid("Editor proposal queue is full"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advisor_errors_preserve_the_existing_monitor_ipc_categories() {
        use openrows_monitor::MonitorError;
        for (error, kind) in [
            (MonitorError::Stopped, IpcErrorKind::NotFound),
            (MonitorError::Limit, IpcErrorKind::Invalid),
            (MonitorError::Encoding, IpcErrorKind::Internal),
            (MonitorError::Unsupported, IpcErrorKind::Unsupported),
        ] {
            assert_eq!(failure(&error).kind, kind);
        }
    }
}

// SPDX-License-Identifier: Apache-2.0
//! Bounded editor proposals contain SQL but have no execution handle.
use crate::connections::{IpcError, IpcErrorKind};
use openrows_mcp::{McpError, ToolInput};
use serde::Serialize;
use specta::Type;
use std::{
    collections::VecDeque,
    sync::{Mutex, PoisonError},
};
use tauri::{AppHandle, Emitter, Manager};

/// Queued model output for normal user editor review.
#[derive(Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SqlProposal {
    connection: String,
    database: String,
    sql: String,
}
#[derive(Default)]
pub(super) struct Proposals(Mutex<VecDeque<SqlProposal>>);
pub(super) fn enqueue(app: &AppHandle, input: &ToolInput) -> Result<(), McpError> {
    let proposal = SqlProposal {
        connection: input.connection.clone().ok_or(McpError::Invalid)?,
        database: input.database.clone().ok_or(McpError::Invalid)?,
        sql: input.sql.clone().ok_or(McpError::Invalid)?,
    };
    let state = app.state::<Proposals>();
    let mut pending = state.0.lock().unwrap_or_else(PoisonError::into_inner);
    if pending.len() >= 16 {
        return Err(McpError::Limit);
    }
    pending.push_back(proposal);
    let _ = app.emit("mcp-proposal-ready", ());
    Ok(())
}
/// Take pending SQL into editor tabs; this command never submits a statement.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri owns command arguments"
)]
#[tauri::command]
#[specta::specta]
pub(crate) fn mcp_proposals_take(app: AppHandle) -> Result<Vec<SqlProposal>, IpcError> {
    app.state::<crate::security::Security>()
        .admit("mcp_proposals_take")?;
    if !app.state::<crate::ai::AiState>().control.enabled() {
        return Err(IpcError::new(
            IpcErrorKind::Unsupported,
            "AI features are off",
        ));
    }
    Ok(app
        .state::<Proposals>()
        .0
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .drain(..)
        .collect())
}

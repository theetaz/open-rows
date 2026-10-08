// SPDX-License-Identifier: Apache-2.0
//! Exact row snapshots for staging: the originals the optimistic guard compares at commit.
use super::dto::TableTarget;
use super::edit_commands::session;
use crate::connections::{Connections, IpcError};
use openrows_driver_api::Session;
use openrows_driver_postgres::PgSession;
use std::collections::BTreeMap;
use tauri::State;

type Keys = BTreeMap<String, String>;
type Snapshot = BTreeMap<String, Option<String>>;

/// Bulk edits read every selected row at once; more than this is not a grid selection.
const MAX_ROWS: usize = 1000;

/// Read exact original values for an identified row; reject unknown/missing keys.
#[tauri::command]
#[specta::specta]
pub(crate) async fn table_edit_row(
    state: State<'_, Connections>,
    target: TableTarget,
    keys: Keys,
) -> Result<Snapshot, IpcError> {
    let mut rows = table_edit_rows(state, target, vec![keys]).await?;
    rows.pop()
        .ok_or_else(|| IpcError::invalid("The row could not be read"))
}

/// Exact original values of several identified rows on one connection, in request order.
#[tauri::command]
#[specta::specta]
pub(crate) async fn table_edit_rows(
    state: State<'_, Connections>,
    target: TableTarget,
    keys: Vec<Keys>,
) -> Result<Vec<Snapshot>, IpcError> {
    if keys.is_empty() || keys.len() > MAX_ROWS {
        return Err(IpcError::invalid("Read between 1 and 1,000 rows at a time"));
    }
    let mut session = session(&state, &target).await?;
    let result = snapshots(&mut session, &target, keys).await;
    let closed = Box::new(session)
        .close()
        .await
        .map_err(|error| IpcError::from(&error));
    closed?;
    result
}

async fn snapshots(
    session: &mut PgSession,
    target: &TableTarget,
    keys: Vec<Keys>,
) -> Result<Vec<Snapshot>, IpcError> {
    let info = session
        .row_edit_info(&target.schema, &target.table)
        .await
        .map_err(|error| IpcError::from(&error))?;
    let complete = |row: &Keys| {
        !info.key.is_empty()
            && info.key.len() == row.len()
            && info.key.iter().all(|name| row.contains_key(name))
    };
    if !keys.iter().all(complete) {
        return Err(IpcError::invalid(
            "This row needs a complete, supported unique key",
        ));
    }
    let mut rows = Vec::with_capacity(keys.len());
    for row in keys {
        let snapshot = session
            .edit_snapshot(
                (&target.schema, &target.table),
                &row.into_iter().collect::<Vec<_>>(),
            )
            .await
            .map_err(|error| IpcError::from(&error))?;
        rows.push(snapshot.into_iter().collect());
    }
    Ok(rows)
}

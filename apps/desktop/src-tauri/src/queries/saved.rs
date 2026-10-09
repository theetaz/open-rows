// SPDX-License-Identifier: Apache-2.0
//! Saved queries of a connection (Run ▸ Add to Queries, the sidebar's
//! Queries tab), kept in the local store.
#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri deserialises command arguments into owned values"
)]
use std::str::FromStr;

use openrows_store::profile::ProfileId;
use openrows_store::{LocalStore, NewSavedQuery, SavedQueryRecord, StoreError};
use serde::Serialize;
use specta::Type;
use tauri::State;

use super::history::now_ms;
use crate::connections::{IpcError, IpcErrorKind};

/// Longest accepted saved-query name, in characters.
const MAX_NAME: usize = 200;

/// A saved query; timestamps are Unix epoch milliseconds.
#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SavedQueryDto {
    id: String,
    name: String,
    sql: String,
    created_at_ms: f64,
    updated_at_ms: f64,
}

#[expect(
    clippy::cast_precision_loss,
    reason = "epoch milliseconds stay far below 2^53"
)]
impl From<SavedQueryRecord> for SavedQueryDto {
    fn from(record: SavedQueryRecord) -> Self {
        Self {
            id: record.id,
            name: record.name,
            sql: record.sql,
            created_at_ms: record.created_at_ms as f64,
            updated_at_ms: record.updated_at_ms as f64,
        }
    }
}

fn storage(error: &StoreError) -> IpcError {
    IpcError::new(IpcErrorKind::Storage, error.to_string())
}

/// The trimmed name, or why it cannot be used.
fn valid_name(name: &str) -> Result<&str, IpcError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(IpcError::invalid("Enter a name for the query"));
    }
    if name.chars().count() > MAX_NAME {
        return Err(IpcError::invalid("Query names have at most 200 characters"));
    }
    Ok(name)
}

fn missing() -> IpcError {
    IpcError::new(IpcErrorKind::NotFound, "The saved query no longer exists")
}

/// Saved queries of a connection, by name.
#[tauri::command]
#[specta::specta]
pub(crate) fn saved_queries_list(
    local: State<'_, LocalStore>,
    profile_id: String,
) -> Result<Vec<SavedQueryDto>, IpcError> {
    let id = ProfileId::from_str(&profile_id)?;
    let saved = local
        .saved_queries(&id.to_string())
        .map_err(|error| storage(&error))?;
    Ok(saved.into_iter().map(SavedQueryDto::from).collect())
}

/// Save SQL under a name on a connection.
#[tauri::command]
#[specta::specta]
pub(crate) fn saved_query_create(
    local: State<'_, LocalStore>,
    profile_id: String,
    name: String,
    sql: String,
) -> Result<SavedQueryDto, IpcError> {
    let id = ProfileId::from_str(&profile_id)?.to_string();
    let query = NewSavedQuery {
        profile_id: &id,
        name: valid_name(&name)?,
        sql: &sql,
        now_ms: now_ms(),
    };
    local
        .create_saved_query(query)
        .map(SavedQueryDto::from)
        .map_err(|error| storage(&error))
}

/// Rename a saved query.
#[tauri::command]
#[specta::specta]
pub(crate) fn saved_query_rename(
    local: State<'_, LocalStore>,
    id: String,
    name: String,
) -> Result<(), IpcError> {
    let renamed = local
        .rename_saved_query(&id, valid_name(&name)?, now_ms())
        .map_err(|error| storage(&error))?;
    if renamed { Ok(()) } else { Err(missing()) }
}

/// Delete a saved query.
#[tauri::command]
#[specta::specta]
pub(crate) fn saved_query_delete(local: State<'_, LocalStore>, id: String) -> Result<(), IpcError> {
    let deleted = local
        .delete_saved_query(&id)
        .map_err(|error| storage(&error))?;
    if deleted { Ok(()) } else { Err(missing()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_trimmed_and_bounded() {
        assert_eq!(valid_name("  weekly  ").ok(), Some("weekly"));
        assert!(valid_name("   ").is_err());
        assert!(valid_name(&"x".repeat(MAX_NAME + 1)).is_err());
        assert!(valid_name(&"é".repeat(MAX_NAME)).is_ok());
    }
}

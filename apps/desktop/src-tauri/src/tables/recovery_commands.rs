// SPDX-License-Identifier: Apache-2.0
//! The local journal of staged table edits (NFR-REL-01, docs/10 §4.5).
//!
//! The webview saves its change set on every mutation and clears it on commit
//! or discard; after a crash the next table open offers the journal back.
//! Values are user data and never leave the local store.
#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri deserialises command arguments into owned values"
)]
use super::dto::TableTarget;
use super::edit_dto::SavedEditsDto;
use crate::connections::{IpcError, IpcErrorKind};
use openrows_store::{ChangeSetRecord, LocalStore, StoreError};
use serde::Serialize;
use specta::Type;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::State;

/// More staged rows than one commit accepts can never be restored usefully.
const MAX_JOURNALED_ROWS: usize = 1000;

/// One journal row per table; the id doubles as the lookup key.
fn journal_id(target: &TableTarget) -> String {
    format!(
        "{}\n{}\n{}\n{}",
        target.profile_id, target.database, target.schema, target.table
    )
}

fn storage(error: &StoreError) -> IpcError {
    IpcError::new(IpcErrorKind::Storage, error.to_string())
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
        })
}

#[expect(
    clippy::cast_precision_loss,
    reason = "epoch milliseconds stay far below 2^53"
)]
fn stamp(ms: i64) -> f64 {
    ms as f64
}

/// Journal the staged edits of a table; an empty change set clears the journal.
#[tauri::command]
#[specta::specta]
pub(crate) fn table_edits_save(
    local: State<'_, LocalStore>,
    target: TableTarget,
    edits: SavedEditsDto,
) -> Result<(), IpcError> {
    if edits.changes.is_empty() {
        return local
            .clear_change_set(&journal_id(&target))
            .map_err(|error| storage(&error));
    }
    if edits.changes.len() > MAX_JOURNALED_ROWS {
        return Err(IpcError::invalid(
            "Only 1,000 staged rows per table are journaled; commit or discard some first",
        ));
    }
    let now = now_ms();
    let saved = SavedEditsDto {
        saved_at_ms: stamp(now),
        ..edits
    };
    let record = ChangeSetRecord {
        id: journal_id(&target),
        profile_id: target.profile_id.clone(),
        target: serde_json::to_string(&target)
            .map_err(|error| IpcError::invalid(error.to_string()))?,
        ops: serde_json::to_string(&saved).map_err(|error| IpcError::invalid(error.to_string()))?,
        updated_at_ms: now,
    };
    local
        .save_change_set(&record)
        .map_err(|error| storage(&error))
}

/// The journaled edits of a table, if any; a journal this build cannot read is an error.
#[tauri::command]
#[specta::specta]
pub(crate) fn table_edits_saved(
    local: State<'_, LocalStore>,
    target: TableTarget,
) -> Result<Option<SavedEditsDto>, IpcError> {
    let Some(record) = local
        .change_set(&journal_id(&target))
        .map_err(|error| storage(&error))?
    else {
        return Ok(None);
    };
    let saved: SavedEditsDto = serde_json::from_str(&record.ops).map_err(|error| {
        IpcError::new(
            IpcErrorKind::Storage,
            format!("The journaled edits of this table cannot be read: {error}"),
        )
    })?;
    Ok(Some(SavedEditsDto {
        saved_at_ms: stamp(record.updated_at_ms),
        ..saved
    }))
}

/// A table with journaled edits: where it is and how many staged rows wait.
#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JournaledEditsDto {
    target: TableTarget,
    rows: u32,
    saved_at_ms: f64,
}

/// Every table with journaled edits, newest first, for the crash-recovery
/// prompt. Values stay in the journal; a journal entry that cannot be read is
/// logged and skipped, since it cannot be restored either.
#[tauri::command]
#[specta::specta]
pub(crate) fn table_edits_journaled(
    local: State<'_, LocalStore>,
) -> Result<Vec<JournaledEditsDto>, IpcError> {
    let records = local.change_sets().map_err(|error| storage(&error))?;
    let mut listed = Vec::with_capacity(records.len());
    for record in &records {
        if let Some(entry) = journaled(record) {
            listed.push(entry);
        } else {
            tracing::warn!(target: "journal", "a journaled table cannot be read");
        }
    }
    Ok(listed)
}

fn journaled(record: &ChangeSetRecord) -> Option<JournaledEditsDto> {
    let target: TableTarget = serde_json::from_str(&record.target).ok()?;
    let saved: SavedEditsDto = serde_json::from_str(&record.ops).ok()?;
    Some(JournaledEditsDto {
        target,
        rows: u32::try_from(saved.changes.len()).unwrap_or(u32::MAX),
        saved_at_ms: stamp(record.updated_at_ms),
    })
}

/// Forget a table's journaled edits (commit, discard, or an unreadable journal).
#[tauri::command]
#[specta::specta]
pub(crate) fn table_edits_clear(
    local: State<'_, LocalStore>,
    target: TableTarget,
) -> Result<(), IpcError> {
    local
        .clear_change_set(&journal_id(&target))
        .map_err(|error| storage(&error))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tables::edit_dto::{CellValueDto, EditKindDto, StagedRowDto};
    use std::collections::BTreeMap;

    fn target(table: &str) -> TableTarget {
        TableTarget {
            profile_id: "p".into(),
            database: "orders".into(),
            schema: "public".into(),
            table: table.into(),
        }
    }

    #[test]
    fn journal_ids_separate_tables_and_round_trip_through_json() -> Result<(), serde_json::Error> {
        assert_ne!(journal_id(&target("users")), journal_id(&target("orders")));
        let edits = SavedEditsDto {
            key: vec!["id".into()],
            changes: vec![StagedRowDto {
                id: r#"{"id":"7"}"#.into(),
                number: 7,
                kind: EditKindDto::Update,
                original: BTreeMap::from([("id".into(), Some("7".into())), ("name".into(), None)]),
                values: BTreeMap::from([(
                    "name".into(),
                    CellValueDto::Text {
                        value: "Ada".into(),
                    },
                )]),
            }],
            saved_at_ms: 0.0,
        };
        let json = serde_json::to_string(&edits)?;
        let back: SavedEditsDto = serde_json::from_str(&json)?;
        assert_eq!(back.key, edits.key);
        assert_eq!(back.changes[0].original, edits.changes[0].original);
        assert_eq!(back.changes[0].values, edits.changes[0].values);
        assert!(
            json.contains(r#""kind":"text""#),
            "values stay tagged: {json}"
        );
        Ok(())
    }
}

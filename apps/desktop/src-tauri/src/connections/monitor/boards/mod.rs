// SPDX-License-Identifier: Apache-2.0
//! Analytics boards (M25-06, docs/28 §5): local per-profile storage,
//! validated import/export and approval of imported boards. Running a
//! widget lives in [`run`].
#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri deserialises command arguments into owned values"
)]
mod protection;
mod run;

pub(crate) use run::*;

use openrows_monitor::boards::{Board, BoardError, admit_sql, export_board, parse_board, validate};
use openrows_sql::Dialect;
use openrows_store::{BoardRecord, LocalStore, NewBoard, StoreError};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::State;

use crate::connections::drivers::driver;
use crate::connections::{Connections, IpcError, IpcErrorKind, profiles};
use crate::queries::history::now_ms;
use crate::security::Security;

/// A board to create (no `id`) or replace.
#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BoardSave {
    profile_id: String,
    id: Option<String>,
    board: Board,
}

/// A stored board; `reviewed` is `false` for an imported board until approved.
#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BoardDto {
    id: String,
    reviewed: bool,
    board: Board,
    updated_at_ms: f64,
}

#[expect(
    clippy::cast_precision_loss,
    reason = "epoch milliseconds stay far below 2^53"
)]
fn dto(record: BoardRecord) -> Result<BoardDto, IpcError> {
    Ok(BoardDto {
        board: parse_board(&record.definition).map_err(|error| board_error(&error))?,
        id: record.id,
        reviewed: record.reviewed,
        updated_at_ms: record.updated_at_ms as f64,
    })
}

pub(super) fn board_error(error: &BoardError) -> IpcError {
    IpcError::invalid(error.to_string())
}

fn storage(error: &StoreError) -> IpcError {
    IpcError::new(IpcErrorKind::Storage, error.to_string())
}

fn missing() -> IpcError {
    IpcError::new(IpcErrorKind::NotFound, "The board no longer exists")
}

/// Canonical profile id and its SQL dialect.
fn profile(state: &Connections, profile_id: &str) -> Result<(String, Dialect), IpcError> {
    let id = profiles::profile_id(profile_id)?;
    let profile = state.profiles.get(id)?;
    Ok((id.to_string(), driver(profile.driver).dialect()))
}

/// Structure plus read-only admission of every widget in `dialect`.
fn checked(board: &Board, dialect: Dialect) -> Result<String, IpcError> {
    validate(board).map_err(|error| board_error(&error))?;
    for widget in &board.widgets {
        admit_sql(&widget.sql, dialect)
            .map_err(|error| IpcError::invalid(format!("{}: {error}", widget.title)))?;
    }
    serde_json::to_string(board).map_err(|_| IpcError::invalid("Board could not be encoded"))
}

/// Stored board `id` of `profile_id`.
pub(super) fn stored(
    local: &LocalStore,
    profile_id: &str,
    id: &str,
) -> Result<BoardRecord, IpcError> {
    let record = local.board(id).map_err(|error| storage(&error))?;
    record
        .filter(|record| record.profile_id == profile_id)
        .ok_or_else(missing)
}

/// Boards of a connection, by name.
#[tauri::command]
#[specta::specta]
pub(crate) fn boards_list(
    security: State<'_, Security>,
    local: State<'_, LocalStore>,
    profile_id: String,
) -> Result<Vec<BoardDto>, IpcError> {
    security.admit("boards_list")?;
    let id = profiles::profile_id(&profile_id)?.to_string();
    let records = local.boards(&id).map_err(|error| storage(&error))?;
    records.into_iter().map(dto).collect()
}

/// Create (no `id`) or replace a board; a board created here is reviewed,
/// a replaced one keeps its review state.
#[tauri::command]
#[specta::specta]
pub(crate) fn board_save(
    security: State<'_, Security>,
    state: State<'_, Connections>,
    local: State<'_, LocalStore>,
    request: BoardSave,
) -> Result<BoardDto, IpcError> {
    security.admit("board_save")?;
    let BoardSave {
        profile_id,
        id,
        board,
    } = request;
    let (profile_id, dialect) = profile(&state, &profile_id)?;
    let definition = checked(&board, dialect)?;
    let Some(id) = id else {
        let new = NewBoard {
            profile_id: &profile_id,
            name: board.name.trim(),
            definition: &definition,
            reviewed: true,
            now_ms: now_ms(),
        };
        return dto(local.create_board(new).map_err(|error| storage(&error))?);
    };
    stored(&local, &profile_id, &id)?;
    let change = (board.name.trim(), definition.as_str());
    local
        .update_board(&id, change, now_ms())
        .map_err(|error| storage(&error))?;
    dto(stored(&local, &profile_id, &id)?)
}

/// Import a portable board; it stays paused until [`board_approve`].
#[tauri::command]
#[specta::specta]
pub(crate) fn board_import(
    security: State<'_, Security>,
    state: State<'_, Connections>,
    local: State<'_, LocalStore>,
    profile_id: String,
    text: String,
) -> Result<BoardDto, IpcError> {
    security.admit("board_import")?;
    let (profile_id, dialect) = profile(&state, &profile_id)?;
    let board = parse_board(&text).map_err(|error| board_error(&error))?;
    let definition = checked(&board, dialect)?;
    let new = NewBoard {
        profile_id: &profile_id,
        name: board.name.trim(),
        definition: &definition,
        reviewed: false,
        now_ms: now_ms(),
    };
    dto(local.create_board(new).map_err(|error| storage(&error))?)
}

/// Portable JSON of a board: definitions only, never results or secrets.
#[tauri::command]
#[specta::specta]
pub(crate) fn board_export(
    security: State<'_, Security>,
    local: State<'_, LocalStore>,
    profile_id: String,
    id: String,
) -> Result<String, IpcError> {
    security.admit("board_export")?;
    let profile_id = profiles::profile_id(&profile_id)?.to_string();
    let board = dto(stored(&local, &profile_id, &id)?)?.board;
    export_board(&board).map_err(|error| board_error(&error))
}

/// The user reviewed an imported board's queries and allows running them.
#[tauri::command]
#[specta::specta]
pub(crate) fn board_approve(
    security: State<'_, Security>,
    local: State<'_, LocalStore>,
    profile_id: String,
    id: String,
) -> Result<(), IpcError> {
    security.admit("board_approve")?;
    let profile_id = profiles::profile_id(&profile_id)?.to_string();
    stored(&local, &profile_id, &id)?;
    let approved = local
        .approve_board(&id, now_ms())
        .map_err(|error| storage(&error))?;
    if approved { Ok(()) } else { Err(missing()) }
}

/// Delete a board.
#[tauri::command]
#[specta::specta]
pub(crate) fn board_delete(
    security: State<'_, Security>,
    local: State<'_, LocalStore>,
    profile_id: String,
    id: String,
) -> Result<(), IpcError> {
    security.admit("board_delete")?;
    let profile_id = profiles::profile_id(&profile_id)?.to_string();
    stored(&local, &profile_id, &id)?;
    let deleted = local.delete_board(&id).map_err(|error| storage(&error))?;
    if deleted { Ok(()) } else { Err(missing()) }
}

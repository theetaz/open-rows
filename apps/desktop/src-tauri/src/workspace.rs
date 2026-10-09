// SPDX-License-Identifier: Apache-2.0
//! Session and workspace state in the local store (M1-21, M1-26, docs/15
//! §2): the webview's JSON for the open query tabs and the open
//! connections, read once on launch and written debounced while the user
//! works. There is one window, `main`, for now.
#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri deserialises command arguments into owned values"
)]
use openrows_store::{LocalStore, StoreError, WorkspaceKey};
use serde::Deserialize;
use specta::Type;
use tauri::State;

use crate::connections::{IpcError, IpcErrorKind};
use crate::queries::history::now_ms;

/// The only window whose state is kept so far.
const MAIN_WINDOW: &str = "main";
/// Largest state accepted; query text of many tabs stays far below.
const MAX_STATE_BYTES: usize = 32 * 1024 * 1024;

/// Which part of the window's state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum WorkspacePartDto {
    /// Open query and table tabs (with query text under Auto Save).
    QueryTabs,
    /// Open connections, their databases and the active one.
    Session,
}

fn key(part: WorkspacePartDto) -> WorkspaceKey<'static> {
    let part = match part {
        WorkspacePartDto::QueryTabs => "queryTabs",
        WorkspacePartDto::Session => "session",
    };
    WorkspaceKey {
        window: MAIN_WINDOW,
        part,
    }
}

fn storage(error: &StoreError) -> IpcError {
    IpcError::new(IpcErrorKind::Storage, error.to_string())
}

/// The saved state of `part`; `null` when nothing is saved.
#[tauri::command]
#[specta::specta]
pub(crate) fn workspace_state_read(
    local: State<'_, LocalStore>,
    part: WorkspacePartDto,
) -> Result<Option<String>, IpcError> {
    local
        .workspace_state(key(part))
        .map_err(|error| storage(&error))
}

/// Save the state of `part`; `null` forgets it (session restore off).
#[tauri::command]
#[specta::specta]
pub(crate) fn workspace_state_write(
    local: State<'_, LocalStore>,
    part: WorkspacePartDto,
    state: Option<String>,
) -> Result<(), IpcError> {
    write(&local, part, state.as_deref())
}

fn write(local: &LocalStore, part: WorkspacePartDto, state: Option<&str>) -> Result<(), IpcError> {
    let result = match state {
        Some(state) if state.len() > MAX_STATE_BYTES => {
            return Err(IpcError::invalid(
                "The workspace state is larger than 32 MiB",
            ));
        }
        Some(state) => local.save_workspace_state(key(part), state, now_ms()),
        None => local.clear_workspace_state(key(part)),
    };
    result.map_err(|error| storage(&error))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parts_are_saved_and_forgotten_separately() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let local = LocalStore::open(&dir.path().join("openrows.sqlite"))?;
        write(&local, WorkspacePartDto::QueryTabs, Some("{\"tabs\":[]}"))?;
        write(&local, WorkspacePartDto::Session, Some("{}"))?;
        write(&local, WorkspacePartDto::Session, None)?;
        let tabs = local.workspace_state(key(WorkspacePartDto::QueryTabs))?;
        assert_eq!(tabs.as_deref(), Some("{\"tabs\":[]}"));
        assert_eq!(local.workspace_state(key(WorkspacePartDto::Session))?, None);
        Ok(())
    }
}

// SPDX-License-Identifier: Apache-2.0
//! Session and workspace state in the local store (M1-21, M1-26, docs/15
//! §2): the webview's JSON for the open query tabs and the open
//! connections, read once on launch and written debounced while the user
//! works. Each workspace window keeps its own state under its label
//! (`main`, `workspace-2`, …; FR-APP-03); the caller's label is taken from
//! the invoking webview, never from the webview's arguments.
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

/// Every stored part, as the store names it.
pub(crate) const PARTS: [&str; 2] = ["queryTabs", "session"];

fn key(window: &str, part: WorkspacePartDto) -> WorkspaceKey<'_> {
    let part = match part {
        WorkspacePartDto::QueryTabs => PARTS[0],
        WorkspacePartDto::Session => PARTS[1],
    };
    WorkspaceKey { window, part }
}

/// The calling workspace window's label; other webviews have no workspace.
fn label(window: &tauri::WebviewWindow) -> Result<&str, IpcError> {
    let label = window.label();
    if crate::windows::is_workspace(label) {
        Ok(label)
    } else {
        Err(IpcError::invalid("This window has no workspace"))
    }
}

fn storage(error: &StoreError) -> IpcError {
    IpcError::new(IpcErrorKind::Storage, error.to_string())
}

/// The saved state of `part`; `null` when nothing is saved.
#[tauri::command]
#[specta::specta]
pub(crate) fn workspace_state_read(
    window: tauri::WebviewWindow,
    local: State<'_, LocalStore>,
    part: WorkspacePartDto,
) -> Result<Option<String>, IpcError> {
    local
        .workspace_state(key(label(&window)?, part))
        .map_err(|error| storage(&error))
}

/// Save the state of `part`; `null` forgets it (session restore off).
#[tauri::command]
#[specta::specta]
pub(crate) fn workspace_state_write(
    window: tauri::WebviewWindow,
    local: State<'_, LocalStore>,
    part: WorkspacePartDto,
    state: Option<String>,
) -> Result<(), IpcError> {
    write(&local, key(label(&window)?, part), state.as_deref())
}

fn write(local: &LocalStore, key: WorkspaceKey<'_>, state: Option<&str>) -> Result<(), IpcError> {
    let result = match state {
        Some(state) if state.len() > MAX_STATE_BYTES => {
            return Err(IpcError::invalid(
                "The workspace state is larger than 32 MiB",
            ));
        }
        Some(state) => local.save_workspace_state(key, state, now_ms()),
        None => local.clear_workspace_state(key),
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
        let tabs = |window| key(window, WorkspacePartDto::QueryTabs);
        let session = |window| key(window, WorkspacePartDto::Session);
        write(&local, tabs("main"), Some("{\"tabs\":[]}"))?;
        write(&local, tabs("workspace-2"), Some("{\"tabs\":[1]}"))?;
        write(&local, session("main"), Some("{}"))?;
        write(&local, session("main"), None)?;
        assert_eq!(
            local.workspace_state(tabs("main"))?.as_deref(),
            Some("{\"tabs\":[]}")
        );
        assert_eq!(
            local.workspace_state(tabs("workspace-2"))?.as_deref(),
            Some("{\"tabs\":[1]}")
        );
        assert_eq!(local.workspace_state(session("main"))?, None);
        Ok(())
    }
}

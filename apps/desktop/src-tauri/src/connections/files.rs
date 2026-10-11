// SPDX-License-Identifier: Apache-2.0
//! "Choose…" for key, certificate and database files: a native open panel
//! (or, for a new `SQLite` file, a save panel) attached to the window. Only
//! the chosen path goes to the webview; files are read by the core when
//! connecting, never by the webview.
use serde::Deserialize;
use specta::Type;
use tauri::State;
use tauri_plugin_dialog::DialogExt as _;

use super::Connections;
use super::error::{IpcError, IpcErrorKind};

/// Which file the connection form asks for.
#[derive(Clone, Copy, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum PickFileKind {
    /// A custom helper executable.
    Program,
    /// SSH private key (opens in `~/.ssh`).
    SshKey,
    /// TLS CA certificate bundle.
    CaCertificate,
    /// TLS client certificate.
    ClientCertificate,
    /// TLS client key.
    ClientKey,
    /// An existing `SQLite` database file.
    SqliteDatabase,
    /// Where to create a new `SQLite` database file (save panel).
    NewSqliteDatabase,
    /// An existing `DuckDB` file.
    DuckdbDatabase,
    /// Where to create a `DuckDB` file.
    NewDuckdbDatabase,
}

impl PickFileKind {
    fn title(self) -> &'static str {
        match self {
            Self::Program => "Choose helper program",
            Self::SshKey => "Choose SSH private key",
            Self::CaCertificate => "Choose CA certificate",
            Self::ClientCertificate => "Choose client certificate",
            Self::ClientKey => "Choose client key",
            Self::SqliteDatabase => "Open SQLite database",
            Self::NewSqliteDatabase => "New SQLite database",
            Self::DuckdbDatabase => "Open DuckDB database",
            Self::NewDuckdbDatabase => "New DuckDB database",
        }
    }
}

/// Pick a key, certificate or database file; `null` when the panel is cancelled.
#[tauri::command]
#[specta::specta]
pub(crate) async fn connection_pick_file(
    window: tauri::WebviewWindow,
    state: State<'_, Connections>,
    kind: PickFileKind,
) -> Result<Option<String>, IpcError> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let mut panel = window
        .dialog()
        .file()
        .set_parent(&window)
        .set_title(kind.title());
    if matches!(kind, PickFileKind::SshKey) {
        panel = panel.set_directory(state.opener.environment.home.join(".ssh"));
    }
    let reply = move |picked| {
        // The receiver only disappears when the command was dropped.
        let _ = sender.send(picked);
    };
    match kind {
        PickFileKind::NewSqliteDatabase => panel.set_file_name("database.sqlite").save_file(reply),
        PickFileKind::NewDuckdbDatabase => panel.set_file_name("database.duckdb").save_file(reply),
        _ => panel.pick_file(reply),
    }
    let picked = receiver
        .await
        .map_err(|_| IpcError::new(IpcErrorKind::Internal, "The file panel closed unexpectedly"))?;
    picked
        .map(|file| {
            file.into_path()
                .map(|path| path.display().to_string())
                .map_err(|error| IpcError::invalid(error.to_string()))
        })
        .transpose()
}

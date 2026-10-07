// SPDX-License-Identifier: Apache-2.0
//! Connection-level open/close: reaching the server before any database is
//! opened, and closing every session of a saved connection.
use serde::Serialize;
use specta::Type;
use tauri::State;

use super::Connections;
use super::error::IpcError;
use super::pg::{self, ServerDto};
use super::profiles::profile_id;

/// A reachable saved connection.
#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConnectionOpened {
    /// The server, read on the connection's home database (the profile's
    /// database, else `postgres`); `server.database` names that database.
    server: ServerDto,
    /// The profile's database to open right away; `None` lets the user choose.
    database: Option<String>,
}

/// Check that a saved connection is reachable and report its server. The
/// check runs on the home database's metadata session, which also lists the
/// server's databases; user sessions open per database with `session_open`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn connection_open(
    state: State<'_, Connections>,
    profile_id: String,
) -> Result<ConnectionOpened, IpcError> {
    let profile = state.profiles.get(self::profile_id(&profile_id)?)?;
    let password = state.profiles.password(&profile)?;
    let target = pg::target(&profile.params, profile.tls.mode, password.as_ref())?;
    let home = target.config.database.clone();
    let server = state
        .metadata
        .read(
            (&profile_id, &home),
            || Ok(target),
            |session| {
                let server = ServerDto::from(session.server_info());
                Box::pin(async move { Ok(server) })
            },
        )
        .await?;
    Ok(ConnectionOpened {
        server,
        database: profile.params.database,
    })
}

/// Close every database session and metadata session of a saved connection.
#[tauri::command]
#[specta::specta]
pub(crate) async fn connection_close(
    state: State<'_, Connections>,
    profile_id: String,
) -> Result<(), IpcError> {
    let id = self::profile_id(&profile_id)?;
    for session in state.owners.unbind_profile(id) {
        // Disconnecting is best effort: a session that already failed is gone.
        let _ = state.sessions.close(session).await;
    }
    state.metadata.close(&profile_id).await;
    Ok(())
}

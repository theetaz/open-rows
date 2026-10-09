// SPDX-License-Identifier: Apache-2.0
//! Connection-level open/close: reaching the server before any database is
//! opened, and closing every session of a saved connection.
use openrows_schema::SchemaCache;
use serde::Serialize;
use specta::Type;
use tauri::State;

use super::Connections;
use super::error::IpcError;
use super::insecure::{InsecureReasonDto, connection_insecurity};
use super::profiles::profile_id;
use super::route;
use super::target::ServerDto;

/// A reachable saved connection.
#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConnectionOpened {
    /// The server, read on the connection's home database (the profile's
    /// database, else the driver's home database: `postgres`,
    /// `information_schema` or `main`); `server.database` names that database.
    server: ServerDto,
    /// The profile's database to open right away; `None` lets the user choose.
    database: Option<String>,
    /// Why the connection is insecure (window chrome badge); empty when secure.
    insecure: Vec<InsecureReasonDto>,
}

/// Check that a saved connection is reachable (opening its SSH tunnel first,
/// with prompts) and report its server. The
/// check runs on the home database's metadata session, which also lists the
/// server's databases; user sessions open per database with `session_open`.
/// Everything cached for the connection turns stale: the tree shows it at
/// once and reads it again in the background.
#[tauri::command]
#[specta::specta]
pub(crate) async fn connection_open(
    state: State<'_, Connections>,
    cache: State<'_, SchemaCache>,
    profile_id: String,
) -> Result<ConnectionOpened, IpcError> {
    let profile = state.profiles.get(self::profile_id(&profile_id)?)?;
    let target = route::profile_target(&state, &profile_id).await?;
    let home = target.database().to_owned();
    let server = state
        .metadata
        .read(
            (&profile_id, &home),
            || async { Ok(target) },
            |connection| {
                let server = ServerDto::from(connection.server());
                Box::pin(async move { Ok(server) })
            },
        )
        .await?;
    cache.invalidate(&profile_id);
    let insecure = connection_insecurity(&profile, state.tunnels.unsaved_host_keys(&profile_id));
    Ok(ConnectionOpened {
        server,
        database: profile.params.database,
        insecure,
    })
}

/// Close every database session and metadata session of a saved connection,
/// and stop sharing its SSH tunnel.
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
    // The tunnel closes once nothing uses it any more.
    state.tunnels.forget(&profile_id);
    Ok(())
}

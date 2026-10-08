// SPDX-License-Identifier: Apache-2.0
//! Per-database session open/close and the session state event.
use std::str::FromStr;

use openrows_core::{SessionEvent, SessionState};
use openrows_driver_api::Session;
use openrows_types::SessionId;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::State;
use tokio::sync::oneshot;

use super::Connections;
use super::error::{IpcError, IpcErrorKind};
use super::owners::Owner;
use super::pg::{self, ServerDto};
use super::route;

/// An open session on one database.
#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionOpened {
    session_id: String,
    server: ServerDto,
}

/// Session lifecycle states the UI shows (docs/04 §3 subset).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum SessionStateDto {
    Connecting,
    Idle,
    Busy,
    Closed,
    Failed,
}

/// Broadcast whenever a session changes state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionStateChanged {
    session_id: String,
    state: SessionStateDto,
    /// Why the session failed, for `failed`.
    message: Option<String>,
}

impl tauri_specta::Event for SessionStateChanged {
    const NAME: &'static str = "session-state-changed";
}

impl From<SessionEvent> for SessionStateChanged {
    fn from(event: SessionEvent) -> Self {
        let state = match event.state {
            SessionState::Connecting => SessionStateDto::Connecting,
            SessionState::Idle => SessionStateDto::Idle,
            SessionState::Busy => SessionStateDto::Busy,
            SessionState::Closed => SessionStateDto::Closed,
            SessionState::Failed => SessionStateDto::Failed,
        };
        Self {
            session_id: event.session.to_string(),
            state,
            message: event.error.map(|error| error.message),
        }
    }
}

/// Open a user session on `database` (the profile's database when `None`);
/// state changes follow as events. `PostgreSQL` cannot switch databases on a
/// connection, so every opened database has its own session.
#[tauri::command]
#[specta::specta]
pub(crate) async fn session_open(
    state: State<'_, Connections>,
    profile_id: String,
    database: Option<String>,
) -> Result<SessionOpened, IpcError> {
    let profile = state
        .profiles
        .get(super::profiles::profile_id(&profile_id)?)?;
    let mut target = route::profile_target(&state, &profile_id).await?;
    if let Some(database) = &database {
        target = target.with_database(database);
    }
    let (server, received) = oneshot::channel();
    let session = state
        .sessions
        .open(async move {
            let connected = pg::connect(&target).await?;
            // `open` awaits this future, so the receiver is still alive.
            let _ = server.send(connected.server);
            let session: Box<dyn Session> = Box::new(connected.session);
            Ok(session)
        })
        .await?;
    let server = received.await.map_err(|_| {
        IpcError::new(
            IpcErrorKind::Internal,
            "The session opened without server info",
        )
    })?;
    let owner = Owner {
        profile: profile.id,
        database: server.database.clone(),
    };
    state.owners.bind(session, owner);
    Ok(SessionOpened {
        session_id: session.to_string(),
        server,
    })
}

/// Cancel the session's work and disconnect; the database's metadata session
/// closes with its last user session.
#[tauri::command]
#[specta::specta]
pub(crate) async fn session_close(
    state: State<'_, Connections>,
    session_id: String,
) -> Result<(), IpcError> {
    let id = SessionId::from_str(&session_id)
        .map_err(|_| IpcError::new(IpcErrorKind::Invalid, "Invalid session id"))?;
    let owner = state.owners.unbind(id);
    let closed = state.sessions.close(id).await;
    if let Some(owner) = owner
        && !state.owners.in_use(owner.profile, &owner.database)
    {
        let profile = owner.profile.to_string();
        state
            .metadata
            .close_database(&profile, &owner.database)
            .await;
    }
    Ok(closed?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use openrows_core::CoreError;
    use openrows_driver_api::DriverError;

    #[test]
    fn failed_events_carry_the_message() -> Result<(), serde_json::Error> {
        let event = SessionStateChanged::from(SessionEvent {
            session: SessionId::new(),
            state: SessionState::Failed,
            error: Some(CoreError::from(&DriverError::Auth(
                "bad password".to_owned(),
            ))),
        });
        let json = serde_json::to_value(event)?;
        assert_eq!(json["state"], "failed");
        assert_eq!(json["message"], "authentication failed: bad password");
        Ok(())
    }
}

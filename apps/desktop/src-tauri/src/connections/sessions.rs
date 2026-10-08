// SPDX-License-Identifier: Apache-2.0
//! Connection test, per-database session open/close and the session state
//! event.
use std::str::FromStr;

use openrows_core::{SessionEvent, SessionState};
use openrows_driver_api::Session;
use openrows_store::profile::{ConnectionProfile, EnvKind, EnvTag, ProfileService};
use openrows_types::SessionId;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::State;
use tokio::sync::oneshot;

use super::Connections;
use super::dto::ProfileInput;
use super::error::{IpcError, IpcErrorKind};
use super::owners::Owner;
use super::pg::{self, ServerDto};
use super::profiles::{profile_id, resolve_password};

/// A successful connection test.
#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TestReport {
    server: ServerDto,
    elapsed_ms: u32,
}

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

/// Driver settings for the form values, using the stored password when the
/// form keeps it.
fn test_target(
    service: &ProfileService,
    id: Option<&str>,
    input: &ProfileInput,
) -> Result<pg::Target, IpcError> {
    let stored = id
        .map(|id| profile_id(id).and_then(|id| Ok(service.get(id)?)))
        .transpose()?;
    let password = resolve_password(service, stored.as_ref(), &input.password)?;
    let mut draft =
        ConnectionProfile::new_postgres(&input.name, input.params(), EnvTag::new(EnvKind::Local));
    input.apply(&mut draft)?;
    pg::target(&draft.params, draft.tls.mode, password.as_ref())
}

/// Connect with the form values, report the server and disconnect.
#[tauri::command]
#[specta::specta]
pub(crate) async fn connection_test(
    state: State<'_, Connections>,
    profile_id: Option<String>,
    input: ProfileInput,
) -> Result<TestReport, IpcError> {
    let target = test_target(&state.profiles, profile_id.as_deref(), &input)?;
    let connected = pg::connect(&target)
        .await
        .map_err(|error| IpcError::from(&error))?;
    let report = TestReport {
        server: connected.server,
        elapsed_ms: u32::try_from(connected.elapsed.as_millis()).unwrap_or(u32::MAX),
    };
    // The test already succeeded; a failed goodbye on a throwaway connection changes nothing.
    let _ = Box::new(connected.session).close().await;
    Ok(report)
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
    let password = state.profiles.password(&profile)?;
    let mut target = pg::target(&profile.params, profile.tls.mode, password.as_ref())?;
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
    use openrows_secrets::{SecretError, SecretKey, SecretStore, SecretString};

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

    /// Fails every call, so any secret store access fails the test.
    struct Untouchable;

    impl SecretStore for Untouchable {
        fn set(&self, _: &SecretKey, _: &SecretString) -> Result<(), SecretError> {
            Err(SecretError::Poisoned)
        }

        fn get(&self, _: &SecretKey) -> Result<Option<SecretString>, SecretError> {
            Err(SecretError::Poisoned)
        }

        fn delete(&self, _: &SecretKey) -> Result<(), SecretError> {
            Err(SecretError::Poisoned)
        }
    }

    #[test]
    fn testing_without_a_stored_password_never_reads_the_keychain()
    -> Result<(), Box<dyn std::error::Error>> {
        use super::super::dto::{EnvKindDto, PasswordInput, tests::input};
        let dir = tempfile::tempdir()?;
        let files = openrows_store::profile::ProfileFiles::new(dir.path());
        let service = ProfileService::new(files, std::sync::Arc::new(Untouchable));
        let saved = super::super::profiles::tests::create_passwordless(&service)?;
        let mut form = input(EnvKindDto::Local);
        let kept = test_target(&service, Some(&saved), &form)?;
        assert!(kept.config.password.is_none());
        form.password = PasswordInput::Set {
            value: "typed-synthetic".to_owned(),
        };
        for id in [None, Some(saved.as_str())] {
            let typed = test_target(&service, id, &form)?;
            assert_eq!(typed.config.password.as_deref(), Some("typed-synthetic"));
        }
        Ok(())
    }
}

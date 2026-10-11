// SPDX-License-Identifier: Apache-2.0
//! One shared session per `DuckDB` profile; no hidden metadata/background connection.
use super::{
    Connections, IpcError, IpcErrorKind,
    owners::Owner,
    target::{ServerDto, Target},
};
use openrows_host_client::{DriverHostRuntime, DuckdbMetadata, HostDriverId, HostedDriver};
use openrows_store::profile::{ConnectionProfile, Driver, ProfileId};
use openrows_types::SessionId;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, PoisonError},
};
use tauri::Manager;

type Held = Option<(SessionId, ServerDto)>;
type Slot = Arc<tokio::sync::Mutex<Held>>;

/// Profile slots serialize competing opens and keep one logical actor/host.
#[derive(Default)]
pub(crate) struct DuckdbSessions(Mutex<HashMap<ProfileId, Slot>>);
impl DuckdbSessions {
    fn slot(&self, profile: ProfileId) -> Slot {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entry(profile)
            .or_default()
            .clone()
    }
    pub(crate) async fn forget(&self, profile: ProfileId) {
        let slot = self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&profile);
        if let Some(slot) = slot {
            slot.lock().await.take();
        }
    }
    pub(crate) async fn forget_session(&self, session: SessionId) {
        let slots = self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .values()
            .cloned()
            .collect::<Vec<_>>();
        for slot in slots {
            let mut held = slot.lock().await;
            if held.as_ref().is_some_and(|(id, _)| *id == session) {
                held.take();
            }
        }
    }
}

/// Attach the lazy installed launcher; native engine code never enters this crate.
pub(crate) fn hosted(
    state: &Connections,
    profile: &ConnectionProfile,
    mut target: Target,
) -> Result<Target, IpcError> {
    if profile.driver == Driver::Duckdb {
        let runtime = state.app.state::<DriverHostRuntime>();
        let launcher = runtime
            .launcher(HostDriverId::Duckdb, &profile.id.to_string())
            .map_err(|error| IpcError::from(&error))?;
        target.hosted = Some(Arc::new(HostedDriver::new(
            launcher,
            Arc::new(DuckdbMetadata),
        )));
    }
    Ok(target)
}

/// Borrow the profile's owning actor; only an explicit connect may replace failure.
pub(crate) async fn shared(
    state: &Connections,
    profile: &str,
    reconnect: bool,
) -> Result<(SessionId, ServerDto), IpcError> {
    let id = super::profiles::profile_id(profile)?;
    let slot = state.duckdb.slot(id);
    let mut held = slot.lock().await;
    if let Some((session, server)) = held.as_ref() {
        let terminal = matches!(
            state.sessions.state(*session),
            None | Some(openrows_core::SessionState::Failed | openrows_core::SessionState::Closed)
        );
        if !terminal {
            return Ok((*session, server.clone()));
        }
        if !reconnect {
            return Err(IpcError::new(
                IpcErrorKind::DriverHost,
                "The DuckDB driver process stopped. Reconnect explicitly; in-memory contents cannot be recovered.",
            ));
        }
        let _ = state.sessions.close(*session).await;
        state.owners.unbind(*session);
        held.take();
    }
    let opened = open(state, profile).await?;
    state.owners.bind(
        opened.0,
        Owner {
            profile: id,
            database: opened.1.database.clone(),
        },
    );
    *held = Some(opened.clone());
    Ok(opened)
}
async fn open(state: &Connections, profile_id: &str) -> Result<(SessionId, ServerDto), IpcError> {
    let profile = state
        .profiles
        .get(super::profiles::profile_id(profile_id)?)?;
    let target = super::route::profile_target(state, profile_id).await?;
    let (server, received) = tokio::sync::oneshot::channel();
    let session = state
        .sessions
        .open(super::resilient::connect_once(target, server))
        .await?;
    state
        .sessions
        .set_statement_timeout(session, state.limits.statement_timeout(&profile))?;
    let server = received
        .await
        .map_err(|_| IpcError::new(IpcErrorKind::Internal, "DuckDB opened without server info"))?;
    Ok((session, server))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn profile_tabs_use_exactly_one_slot() {
        let sessions = DuckdbSessions::default();
        let id = ProfileId::new();
        assert!(Arc::ptr_eq(&sessions.slot(id), &sessions.slot(id)));
        assert!(!Arc::ptr_eq(
            &sessions.slot(id),
            &sessions.slot(ProfileId::new())
        ));
    }
}

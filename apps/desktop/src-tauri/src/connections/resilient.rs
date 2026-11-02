// SPDX-License-Identifier: Apache-2.0
//! Resilient user sessions and the path monitor (M1-08, docs/24 §6).
//!
//! A server session keeps its resolved [`Target`] (route, database, TLS and
//! secrets), so the core can reopen it after a loss; a profile with SSH dials
//! through its shared tunnel, which reconnects itself. After the machine
//! wakes or the network changes, every tunnel is pinged (dead ones are
//! dropped so the next dial reconnects), idle metadata connections are
//! dropped and every session probes its connection at once. A session on a
//! short-lived cloud token or a credential command gets a new password for
//! each connect: its recipe holds one only until the first connect uses it.
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use openrows_core::{Reconnect, Resilience, RetryPolicy};
use openrows_driver_api::{BoxFuture, DriverError, Session};
use openrows_net::{SecretString, watch_path};
use openrows_sql::Dialect;
use tauri::{AppHandle, Manager};
use tokio::sync::oneshot;

use super::Connections;
use super::target::{self, ServerDto, Target};

/// How often the path monitor samples clocks and routes.
const PATH_TICK: Duration = Duration::from_secs(2);
/// How long a tunnel may take to answer after wake before it is dropped.
const TUNNEL_PROBE: Duration = Duration::from_secs(5);

type FirstServer = Arc<Mutex<Option<oneshot::Sender<ServerDto>>>>;

/// Fetches a new password for every connect after the first.
pub(crate) type FreshPassword =
    Arc<dyn Fn() -> BoxFuture<'static, Result<SecretString, DriverError>> + Send + Sync>;

/// The password of the next connect: the one the session opened with, once,
/// then always a new one, so an expired token is never replayed.
pub(super) struct OneTimePassword {
    first: Mutex<Option<SecretString>>,
    fresh: FreshPassword,
}

impl OneTimePassword {
    pub(super) fn new(first: Option<SecretString>, fresh: FreshPassword) -> Self {
        Self {
            first: Mutex::new(first),
            fresh,
        }
    }

    pub(super) async fn next(&self) -> Result<SecretString, DriverError> {
        let first = self
            .first
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        match first {
            Some(password) => Ok(password),
            None => (self.fresh)().await,
        }
    }
}

/// Connect to `target`; the first successful connect reports its server.
async fn connect(target: &Target, first: &FirstServer) -> Result<Box<dyn Session>, DriverError> {
    let connected = target::connect(target).await?;
    let sender = first.lock().unwrap_or_else(PoisonError::into_inner).take();
    if let Some(sender) = sender {
        // `session_open` awaits the first connect, so the receiver is alive.
        let _ = sender.send(connected.server);
    }
    Ok(connected.connection)
}

/// Connect once (local files), reporting the server to `server`.
pub(crate) async fn connect_once(
    target: Target,
    server: oneshot::Sender<ServerDto>,
) -> Result<Box<dyn Session>, DriverError> {
    connect(&target, &Arc::new(Mutex::new(Some(server)))).await
}

/// Reopen `target` after every loss, replaying `dialect`'s search-path and
/// database statements; the first connect reports its server to `server`.
/// With `fresh`, each connect after the first uses a newly fetched password.
pub(crate) fn recipe(
    mut target: Target,
    dialect: Dialect,
    server: oneshot::Sender<ServerDto>,
    fresh: Option<FreshPassword>,
) -> Resilience {
    let password =
        fresh.map(|fresh| Arc::new(OneTimePassword::new(target.request.password.take(), fresh)));
    let target = Arc::new(target);
    let first: FirstServer = Arc::new(Mutex::new(Some(server)));
    let reconnect: Reconnect = Arc::new(move || {
        let (target, first, password) = (Arc::clone(&target), Arc::clone(&first), password.clone());
        Box::pin(async move {
            let Some(password) = password else {
                return connect(&target, &first).await;
            };
            let mut attempt = (*target).clone();
            attempt.request.password = Some(password.next().await?);
            connect(&attempt, &first).await
        })
    });
    Resilience {
        reconnect,
        dialect: Some(dialect),
        policy: RetryPolicy::default(),
        setup: Vec::new(),
    }
}

#[cfg(test)]
#[path = "resilient_tests.rs"]
mod tests;

/// Watch for sleep/wake and network changes for the app's lifetime.
pub(crate) fn watch_network(app: AppHandle) {
    tauri::async_runtime::spawn(watch_path(PATH_TICK, move |_change| {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            reprobe(&app.state::<Connections>()).await;
        });
    }));
}

/// Probe tunnels first (so sessions dial a live one), then sessions.
async fn reprobe(state: &Connections) {
    state.monitors.stop_all();
    state.tunnels.probe_all(TUNNEL_PROBE).await;
    state.metadata.reset_idle();
    state.sessions.probe_all();
}

// SPDX-License-Identifier: Apache-2.0
//! Resilient user sessions and the path monitor (M1-08, docs/24 §6).
//!
//! A server session keeps its resolved [`Target`] (route, database, TLS and
//! secrets), so the core can reopen it after a loss; a profile with SSH dials
//! through its shared tunnel, which reconnects itself. After the machine
//! wakes or the network changes, every tunnel is pinged (dead ones are
//! dropped so the next dial reconnects), idle metadata connections are
//! dropped and every session probes its connection at once.
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use openrows_core::{Reconnect, Resilience, RetryPolicy};
use openrows_driver_api::{DriverError, Session};
use openrows_net::watch_path;
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
pub(crate) fn recipe(
    target: Target,
    dialect: Dialect,
    server: oneshot::Sender<ServerDto>,
) -> Resilience {
    let target = Arc::new(target);
    let first: FirstServer = Arc::new(Mutex::new(Some(server)));
    let reconnect: Reconnect = Arc::new(move || {
        let (target, first) = (Arc::clone(&target), Arc::clone(&first));
        Box::pin(async move { connect(&target, &first).await })
    });
    Resilience {
        reconnect,
        dialect: Some(dialect),
        policy: RetryPolicy::default(),
        setup: Vec::new(),
    }
}

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

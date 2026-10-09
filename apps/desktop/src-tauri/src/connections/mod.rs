// SPDX-License-Identifier: Apache-2.0
//! Connection profiles and sessions exposed to the webview (D-05).
//!
//! Startup opens the local store and profiles in the app-data dir (docs/15
//! §1), the OS keychain (behind a per-run read cache) and a
//! [`SessionManager`] that streams result batches into the shared
//! [`ResultStore`]; commands live in [`profiles`] and
//! [`sessions`]; [`catalog`] serves the schema tree from per-database
//! metadata sessions. Each opened database of a connection has its own user
//! session, recorded in [`owners`]; [`catalog_cache`] keeps their listings
//! (with persisted snapshots) in the managed [`SchemaCache`]. [`route`] resolves every connect: keychain
//! secrets, TLS files and the profile's shared SSH tunnel ([`tunnels`]) into
//! a driver [`target`] for the profile's engine ([`drivers`]), with
//! host-key and keyboard-interactive [`prompts`] in the webview;
//! [`diagnostics`] runs Test connection step by step; [`attach`] attaches
//! and detaches `SQLite` database files. Server sessions are [`resilient`]:
//! they probe and reconnect, also after wake.
pub(crate) mod attach;
pub(crate) mod catalog;
pub(crate) mod catalog_cache;
mod catalog_dto;
pub(crate) mod connection;
pub(crate) mod diagnostics;
pub(crate) mod drivers;
mod dto;
mod engine_dto;
mod enums;
mod error;
pub(crate) mod files;
mod insecure;
mod metadata;
mod owners;
pub(crate) mod profiles;
pub(crate) mod prompts;
mod proxy_dto;
mod resilient;
pub(crate) mod route;
mod route_dto;
pub(crate) mod sessions;
pub(crate) mod target;
mod tunnels;

use std::sync::Arc;

use openrows_core::SessionManager;
use openrows_results::ResultStore;
use openrows_schema::SchemaCache;
use openrows_secrets::{CachingSecretStore, KeychainSecretStore};
use openrows_store::LocalStore;
use openrows_store::profile::{ProfileFiles, ProfileService};
use tauri::Manager;
use tauri_specta::Event;
use tokio::sync::broadcast::error::RecvError;

use crate::queries::history::now_ms;
pub(crate) use error::{IpcError, IpcErrorKind};
use prompts::{PromptBroker, TauriSink};
use sessions::SessionStateChanged;
use tunnels::{SshEnvironment, TunnelOpener, Tunnels};

/// Schema snapshots unused (not refreshed) for 60 days are evicted (docs/15 §2).
const SNAPSHOT_RETENTION_MS: i64 = 60 * 24 * 60 * 60 * 1000;

/// Managed state behind every connection and query command.
pub(crate) struct Connections {
    pub(crate) profiles: ProfileService,
    pub(crate) sessions: SessionManager,
    metadata: metadata::MetadataSessions,
    /// Which saved connection and database each open session belongs to.
    pub(crate) owners: owners::Owners,
    /// Shared SSH tunnels per saved connection.
    pub(crate) tunnels: Tunnels,
    /// Plans and opens SSH tunnels with prompts in the webview.
    pub(crate) opener: TunnelOpener,
    /// Open host-key and keyboard-interactive prompts.
    pub(crate) prompts: Arc<PromptBroker>,
}

/// Open the stores, manage [`Connections`] and forward session events.
/// Requires the managed `Arc<ResultStore>` (see `result_store::install`).
///
/// # Errors
/// The app-data dir cannot be resolved or created, or the database fails to open.
pub(crate) fn install(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let data = app.path().app_data_dir()?;
    std::fs::create_dir_all(&data)?;
    let store = LocalStore::open(&data.join("openrows.sqlite"))?;
    // Eviction is housekeeping: snapshots that stay only cost disk space.
    let _ = store.prune_schema_snapshots(now_ms() - SNAPSHOT_RETENTION_MS);
    app.manage(store);
    app.manage(SchemaCache::default());
    let profiles = ProfileService::new(
        ProfileFiles::new(&data.join("connections")),
        // Read each keychain item once per run: every read by a binary the
        // item's ACL does not trust is a macOS password prompt.
        Arc::new(CachingSecretStore::new(KeychainSecretStore::new())),
    );
    let results = Arc::clone(app.state::<Arc<ResultStore>>().inner());
    let sessions = SessionManager::new(results);
    forward_session_events(app.handle().clone(), &sessions);
    resilient::watch_network(app.handle().clone());
    let prompts = Arc::new(PromptBroker::new(Box::new(TauriSink(app.handle().clone()))));
    let environment = SshEnvironment {
        home: app.path().home_dir()?,
        // App-managed known_hosts next to the profiles; written 0600.
        learned_hosts: data.join("known_hosts"),
        local_user: local_user(),
    };
    app.manage(Connections {
        profiles,
        sessions,
        metadata: metadata::MetadataSessions::default(),
        owners: owners::Owners::default(),
        tunnels: Tunnels::default(),
        opener: TunnelOpener {
            environment,
            prompts: Arc::clone(&prompts),
        },
        prompts,
    });
    Ok(())
}

/// The macOS login name, the SSH user when nothing else names one.
fn local_user() -> String {
    ["USER", "LOGNAME"]
        .iter()
        .find_map(|name| std::env::var(name).ok().filter(|user| !user.is_empty()))
        .unwrap_or_default()
}

fn forward_session_events(app: tauri::AppHandle, sessions: &SessionManager) {
    let mut events = sessions.subscribe();
    tauri::async_runtime::spawn(async move {
        loop {
            match events.recv().await {
                Ok(event) => {
                    // Emitting only fails while the webview is gone; the next event retries.
                    let _ = SessionStateChanged::from(event).emit(&app);
                }
                Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => break,
            }
        }
    });
}

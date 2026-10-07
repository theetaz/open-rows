// SPDX-License-Identifier: Apache-2.0
//! Connection profiles and sessions exposed to the webview (D-05).
//!
//! Startup opens the local store and profiles in the app-data dir (docs/15
//! §1), the OS keychain and a [`SessionManager`] that streams result batches
//! into the shared [`ResultStore`]; commands live in [`profiles`] and
//! [`sessions`]; [`catalog`] serves the schema tree from per-database
//! metadata sessions. Each opened database of a connection has its own user
//! session, recorded in [`owners`].
pub(crate) mod catalog;
mod catalog_dto;
pub(crate) mod connection;
mod dto;
mod enums;
mod error;
mod metadata;
mod owners;
pub(crate) mod pg;
pub(crate) mod profiles;
pub(crate) mod sessions;

use std::sync::Arc;

use openrows_core::SessionManager;
use openrows_results::ResultStore;
use openrows_secrets::KeychainSecretStore;
use openrows_store::LocalStore;
use openrows_store::profile::{ProfileFiles, ProfileService};
use tauri::Manager;
use tauri_specta::Event;
use tokio::sync::broadcast::error::RecvError;

pub(crate) use error::{IpcError, IpcErrorKind};
use sessions::SessionStateChanged;

/// Managed state behind every connection and query command.
pub(crate) struct Connections {
    pub(crate) profiles: ProfileService,
    pub(crate) sessions: SessionManager,
    metadata: metadata::MetadataSessions,
    /// Which saved connection and database each open session belongs to.
    pub(crate) owners: owners::Owners,
}

/// Open the stores, manage [`Connections`] and forward session events.
/// Requires the managed `Arc<ResultStore>` (see `spike::configure`).
///
/// # Errors
/// The app-data dir cannot be resolved or created, or the database fails to open.
pub(crate) fn install(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let data = app.path().app_data_dir()?;
    std::fs::create_dir_all(&data)?;
    app.manage(LocalStore::open(&data.join("openrows.sqlite"))?);
    let profiles = ProfileService::new(
        ProfileFiles::new(&data.join("connections")),
        Arc::new(KeychainSecretStore::new()),
    );
    let results = Arc::clone(app.state::<Arc<ResultStore>>().inner());
    let sessions = SessionManager::new(results);
    forward_session_events(app.handle().clone(), &sessions);
    app.manage(Connections {
        profiles,
        sessions,
        metadata: metadata::MetadataSessions::default(),
        owners: owners::Owners::default(),
    });
    Ok(())
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

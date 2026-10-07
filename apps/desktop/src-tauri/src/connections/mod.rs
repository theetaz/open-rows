// SPDX-License-Identifier: Apache-2.0
//! Connection profiles and sessions exposed to the webview (D-05).
//!
//! Startup opens the local store and profiles in the app-data dir (docs/15
//! §1), the OS keychain and a [`SessionManager`]; commands live in
//! [`profiles`] and [`sessions`]; [`catalog`] serves the schema tree from
//! per-database metadata sessions.
pub(crate) mod catalog;
mod catalog_dto;
mod dto;
mod enums;
mod error;
mod metadata;
mod pg;
pub(crate) mod profiles;
pub(crate) mod sessions;

use std::sync::Arc;

use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use openrows_core::{BatchSink, SessionManager};
use openrows_driver_api::SinkFlow;
use openrows_secrets::KeychainSecretStore;
use openrows_store::LocalStore;
use openrows_store::profile::{ProfileFiles, ProfileService};
use openrows_types::ResultId;
use tauri::Manager;
use tauri_specta::Event;
use tokio::sync::broadcast::error::RecvError;

use sessions::SessionStateChanged;

/// Managed state behind every connection command.
pub(crate) struct Connections {
    profiles: ProfileService,
    sessions: SessionManager,
    metadata: metadata::MetadataSessions,
}

/// Drops result batches until the result store is wired to sessions (D-07);
/// no command executes statements yet.
struct NoResults;

impl BatchSink for NoResults {
    fn begin(&self, _result: ResultId, _schema: SchemaRef) {}

    fn push(&self, _result: ResultId, _batch: RecordBatch) -> SinkFlow {
        SinkFlow::Stop
    }

    fn end(&self, _result: ResultId) {}
}

/// Open the stores, manage [`Connections`] and forward session events.
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
    let sessions = SessionManager::new(Arc::new(NoResults));
    forward_session_events(app.handle().clone(), &sessions);
    app.manage(Connections {
        profiles,
        sessions,
        metadata: metadata::MetadataSessions::default(),
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

// SPDX-License-Identifier: Apache-2.0
//! Thin monitoring IPC and profile lifecycle ownership; no monitor UI.
mod advisor;
mod commands;
mod diagnostic;
mod diagnostic_gate;
mod diagnostic_session;
mod hypopg;
pub(crate) use advisor::*;
pub(crate) use diagnostic::*;
pub(crate) use hypopg::*;
mod history;
pub(crate) use history::*;
pub(crate) mod processes;
mod reader;
mod registry;
pub(crate) use commands::*;
use openrows_monitor::{Descriptor, Source, SourceStatus};
pub(crate) use registry::Monitors;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri_specta::Event;

/// One connection/database scope shared by every subscriber.
#[derive(Clone, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MonitorTarget {
    /// Saved profile identity.
    pub(crate) profile_id: String,
    /// Explicit selected database.
    pub(crate) database: String,
}
impl MonitorTarget {
    fn key(&self) -> registry::Key {
        (
            registry::profile_key(&self.profile_id),
            self.database.clone(),
        )
    }
}
/// Explicit start/resume; does not restore collection on startup.
#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MonitorStart {
    /// Monitor scope.
    target: MonitorTarget,
    /// Live cadence; default five seconds.
    interval_ms: Option<u32>,
}
/// Bounded binary source/top-N/live-series window request.
#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MonitorWindow {
    /// Selected connection/database.
    target: MonitorTarget,
    /// None reads scalar series, otherwise a latest source table.
    source: Option<Source>,
    /// Rows, up to 1,000; statements up to 100, default 20.
    limit: Option<u32>,
    /// Stable snapshot offset; pages are capped at 1,000 rows.
    row_start: Option<u32>,
    /// Rank statements by live-window weighted mean rather than total time.
    mean: Option<bool>,
}
/// Per-generation source status; no sampled rows in events.
#[derive(Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MonitorChanged {
    target: MonitorTarget,
    generation: String,
    sources: Vec<SourceStatus>,
}
impl Event for MonitorChanged {
    const NAME: &'static str = "monitor-changed";
}
/// Admission reply and small current snapshot.
#[derive(Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MonitorReply {
    generation: String,
    capabilities: Vec<Descriptor>,
    sources: Vec<SourceStatus>,
}

/// Canonical history profile identity, shared with collection.
pub(crate) fn profile_key_for_history(id: &str) -> String {
    registry::profile_key(id)
}

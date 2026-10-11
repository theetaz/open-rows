// SPDX-License-Identifier: Apache-2.0
//! Local aggregate history, independent of collector admission and never raw SQL.
#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri command inputs and map_err failures are owned"
)]
use super::MonitorTarget;
use crate::connections::{Connections, IpcError, IpcErrorKind};
use openrows_monitor::Monitor;
use openrows_store::{LocalStore, MonitorAggregate};
use serde::Serialize;
use specta::Type;
use tauri::{Manager, State};

/// Bounded persisted scalar observation; no database identity, text or server error.
#[derive(Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HistoryPoint {
    metric: String,
    collected_ms: f64,
    value: Option<f64>,
    state: String,
}
impl From<MonitorAggregate> for HistoryPoint {
    fn from(point: MonitorAggregate) -> Self {
        Self {
            metric: point.metric,
            collected_ms: millis(point.collected_ms),
            value: point.value,
            state: point.state,
        }
    }
}
#[expect(
    clippy::cast_precision_loss,
    reason = "wall milliseconds fit exact f64 integers"
)]
fn millis(value: i64) -> f64 {
    value as f64
}

/// Read a selected range from the profile's scalar history; fixed 24-hour retention.
#[tauri::command]
#[specta::specta]
pub(crate) fn monitor_history(
    state: State<'_, Connections>,
    target: MonitorTarget,
    hours: u32,
) -> Result<Vec<HistoryPoint>, IpcError> {
    state
        .app
        .state::<crate::security::Security>()
        .admit("monitor_history")?;
    let now = crate::queries::history::now_ms();
    let points = state
        .app
        .state::<LocalStore>()
        .monitor_history(
            (
                &super::registry::profile_key(&target.profile_id),
                &target.database,
            ),
            (
                now.saturating_sub(i64::from(hours.clamp(1, 24)) * 3_600_000),
                now,
            ),
        )
        .map_err(storage)?;
    Ok(points.into_iter().map(HistoryPoint::from).collect())
}

/// Apply explicit per-profile opt-in; disabling purges before this command returns.
#[tauri::command]
#[specta::specta]
pub(crate) fn monitor_history_configure(
    state: State<'_, Connections>,
    profiles: Vec<String>,
) -> Result<(), IpcError> {
    state
        .app
        .state::<crate::security::Security>()
        .admit("monitor_history_configure")?;
    let profiles = profiles
        .iter()
        .map(|id| super::super::profiles::profile_id(id).map(|id| id.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    state
        .app
        .state::<LocalStore>()
        .set_monitor_profiles(&profiles, crate::queries::history::now_ms())
        .map_err(storage)
}

/// Purge a profile or all local monitor history, leaving the opt-in choice intact.
#[tauri::command]
#[specta::specta]
pub(crate) fn monitor_history_purge(
    state: State<'_, Connections>,
    profile_id: Option<String>,
) -> Result<(), IpcError> {
    state
        .app
        .state::<crate::security::Security>()
        .admit("monitor_history_purge")?;
    let profile = profile_id.map(|id| super::registry::profile_key(&id));
    state
        .app
        .state::<LocalStore>()
        .purge_monitor_history(profile.as_deref())
        .map_err(storage)
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "core emits finite wall-clock milliseconds within i64"
)]
pub(super) fn persist(
    app: &tauri::AppHandle,
    target: &MonitorTarget,
    monitor: &Monitor,
) -> Result<(), IpcError> {
    let now = crate::queries::history::now_ms();
    let points: Vec<_> = monitor
        .aggregates()
        .into_iter()
        .filter(|(_, time, _, _)| *time >= millis(now.saturating_sub(86_400_000)))
        .map(|(metric, collected_ms, value, state)| MonitorAggregate {
            metric,
            collected_ms: collected_ms as i64,
            value,
            state,
        })
        .collect();
    app.state::<LocalStore>()
        .record_monitor((&target.profile_id, &target.database), &points, now)
        .map_err(storage)
}
fn storage(error: openrows_store::StoreError) -> IpcError {
    IpcError::new(IpcErrorKind::Storage, error.to_string())
}

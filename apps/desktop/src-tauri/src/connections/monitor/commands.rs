// SPDX-License-Identifier: Apache-2.0
//! Explicit admission/control and bounded binary windows.
use super::{MonitorChanged, MonitorReply, MonitorStart, MonitorTarget, MonitorWindow, reader};
use crate::connections::{Connections, IpcError, IpcErrorKind};
use openrows_monitor::{Descriptor, Monitor, MonitorError, MonitorReader, Source};
use std::{
    sync::{Arc, PoisonError},
    time::Duration,
};
use tauri::{Manager, State};
use tauri_specta::Event;

/// Start/resume one shared collector, requiring an already connected profile.
#[tauri::command]
#[specta::specta]
pub(crate) async fn monitor_start(
    state: State<'_, Connections>,
    request: MonitorStart,
) -> Result<MonitorReply, IpcError> {
    let interval = request.interval_ms.unwrap_or(5_000);
    if ![2_000, 5_000, 10_000, 30_000, 60_000].contains(&interval) {
        return Err(IpcError::invalid("Choose 2, 5, 10, 30 or 60 seconds"));
    }
    let key = request.target.key();
    state
        .app
        .state::<crate::security::Security>()
        .admit("monitor_start")?;
    let background = state.monitors.background(&key);
    if let Ok(monitor) = state.monitors.get(&key)
        && !monitor.reusable(interval)
    {
        state.monitors.stop(&key);
    }
    let (slot, fresh) = state.monitors.reserve(&key).map_err(error)?;
    if !fresh {
        return reply(&state, &request.target);
    }
    let mut cancelled = slot
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .cancel
        .subscribe();
    let opened = tokio::select! {
        result = prepare(&state, &request.target, interval) => result,
        _ = cancelled.changed() => Err(error(MonitorError::Stopped)),
    };
    let monitor = match opened {
        Ok(monitor) => Arc::new(monitor),
        Err(failure) => {
            state.monitors.stop_reserved(&key, &slot);
            return Err(failure);
        }
    };
    if let Err(failure) = state
        .app
        .state::<crate::security::Security>()
        .admit("monitor_start")
    {
        monitor.stop();
        state.monitors.stop_reserved(&key, &slot);
        return Err(failure);
    }
    let mut slot = slot.lock().unwrap_or_else(PoisonError::into_inner);
    if *slot.cancel.borrow() {
        monitor.stop();
        return Err(error(MonitorError::Stopped));
    }
    forward(&state.app, &request.target, &monitor);
    slot.monitor = Some(monitor.clone());
    drop(slot);
    if background {
        // A cadence change restarts the generation without dropping the opt-in.
        state.monitors.set_background(&key, true).map_err(error)?;
    }
    reply(&state, &request.target)
}

async fn prepare(
    state: &Connections,
    target: &MonitorTarget,
    interval: u32,
) -> Result<Monitor, IpcError> {
    let mut reader = reader::open(state, target).await?;
    let descriptors = tokio::time::timeout(Duration::from_secs(16), reader.discover())
        .await
        .map_err(|_| {
            IpcError::new(
                IpcErrorKind::Timeout,
                "Monitoring discovery timed out; session closed",
            )
        })?;
    Monitor::spawn(Box::new(reader), descriptors, interval).map_err(error)
}

/// Close a view: stop and discard samples unless background monitoring is opted in.
#[tauri::command]
#[specta::specta]
pub(crate) async fn monitor_stop(
    state: State<'_, Connections>,
    target: MonitorTarget,
) -> Result<(), IpcError> {
    state.monitors.release(&target.key());
    Ok(())
}

/// Pause clears the generation and background opt-in; explicit start re-probes.
#[tauri::command]
#[specta::specta]
pub(crate) async fn monitor_pause(
    state: State<'_, Connections>,
    target: MonitorTarget,
) -> Result<(), IpcError> {
    state.monitors.stop(&target.key());
    Ok(())
}

/// List source capabilities, scope and reviewed setup guidance.
#[tauri::command]
#[specta::specta]
pub(crate) async fn monitor_capabilities(
    state: State<'_, Connections>,
    target: MonitorTarget,
) -> Result<Vec<Descriptor>, IpcError> {
    Ok(state
        .monitors
        .get(&target.key())
        .map_err(error)?
        .capabilities())
}

/// Read only small statuses and freshness; sampled rows stay binary.
#[tauri::command]
#[specta::specta]
pub(crate) async fn monitor_snapshot(
    state: State<'_, Connections>,
    target: MonitorTarget,
) -> Result<MonitorReply, IpcError> {
    reply(&state, &target)
}

/// Arrow source table or scalar live-series window, capped at one MiB.
#[tauri::command]
pub(crate) async fn monitor_window(
    state: State<'_, Connections>,
    request: MonitorWindow,
) -> Result<tauri::ipc::Response, IpcError> {
    bytes(&state, &request)
}

/// Top statements by covered live-window total/weighted mean, never raw text.
#[tauri::command]
pub(crate) async fn monitor_top_statements(
    state: State<'_, Connections>,
    mut request: MonitorWindow,
) -> Result<tauri::ipc::Response, IpcError> {
    request.source = Some(Source::Statements);
    bytes(&state, &request)
}

fn bytes(state: &Connections, request: &MonitorWindow) -> Result<tauri::ipc::Response, IpcError> {
    state
        .monitors
        .get(&request.target.key())
        .map_err(error)?
        .windowed(openrows_monitor::WindowOptions {
            source: request.source,
            row_start: request.row_start.unwrap_or(0),
            row_count: request
                .limit
                .unwrap_or(if request.source == Some(Source::Statements) {
                    20
                } else {
                    1_000
                }),
            mean: request.mean.unwrap_or(false),
        })
        .map(tauri::ipc::Response::new)
        .map_err(error)
}
fn reply(state: &Connections, target: &MonitorTarget) -> Result<MonitorReply, IpcError> {
    let monitor = state.monitors.get(&target.key()).map_err(error)?;
    Ok(MonitorReply {
        generation: generation(&monitor),
        background: state.monitors.background(&target.key()),
        capabilities: monitor.capabilities(),
        sources: monitor.snapshot(),
    })
}
fn generation(monitor: &Arc<Monitor>) -> String {
    monitor.generation()
}
fn forward(app: &tauri::AppHandle, target: &MonitorTarget, monitor: &Arc<Monitor>) {
    let (app, target, generation) = (app.clone(), target.clone(), generation(monitor));
    let mut target = target;
    target.profile_id = super::registry::profile_key(&target.profile_id);
    let weak = Arc::downgrade(monitor);
    let mut events = monitor.subscribe();
    tauri::async_runtime::spawn(async move {
        while let Ok(sources) = events.recv().await {
            let Some(monitor) = weak.upgrade() else {
                break;
            };
            if let Err(error) = super::history::persist(&app, &target, &monitor) {
                tracing::warn!(kind = ?error.kind, "monitor history write failed");
            }
            super::alerts::evaluate(&app, &target, &monitor);
            let _ = MonitorChanged {
                target: target.clone(),
                generation: generation.clone(),
                sources,
            }
            .emit(&app);
        }
    });
}
#[expect(
    clippy::needless_pass_by_value,
    reason = "map_err consumes typed failures at the IPC boundary"
)]
pub(super) fn error(error: MonitorError) -> IpcError {
    let kind = match error {
        MonitorError::Stopped => IpcErrorKind::NotFound,
        MonitorError::Limit => IpcErrorKind::Invalid,
        MonitorError::Encoding => IpcErrorKind::Internal,
        MonitorError::Unsupported => IpcErrorKind::Unsupported,
    };
    IpcError::new(kind, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn binary_requests_and_explicit_missing_states_match_the_contract()
    -> Result<(), serde_json::Error> {
        let request: MonitorWindow = serde_json::from_value(
            serde_json::json!({"target":{"profileId":"p","database":"db"},"source":"statements","limit":20,"mean":true}),
        )?;
        assert_eq!(request.source, Some(Source::Statements));
        let state = openrows_monitor::SampleState::PermissionDenied("pg_read_all_stats".into());
        assert_eq!(serde_json::to_value(state)?["kind"], "permissionDenied");
        Ok(())
    }
}

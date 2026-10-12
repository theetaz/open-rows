// SPDX-License-Identifier: Apache-2.0
//! Session opt-in recurring jobs. Lock/disconnect cancels and requires another explicit enable.
mod run;
use super::{AssistantRequest, context};
use crate::connections::IpcError;
use openrows_ai::{Cancellation, scheduled::InvestigationSchedule, sharing::InvestigationAudit};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Mutex, PoisonError},
};
use tauri::{AppHandle, Manager};

#[derive(Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScheduledResult {
    audit: InvestigationAudit,
    events: Vec<openrows_ai::agent::AgentEvent>,
}
#[derive(Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScheduledJob {
    id: String,
    connection: String,
    database: String,
    config: InvestigationSchedule,
    running: bool,
    failure: Option<String>,
    results: VecDeque<ScheduledResult>,
}
struct Job {
    view: ScheduledJob,
    cancel: Cancellation,
}
#[derive(Default)]
pub(crate) struct ScheduledState(Mutex<BTreeMap<String, Arc<Mutex<Job>>>>);
#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ScheduleRequest {
    request: AssistantRequest,
    config: InvestigationSchedule,
}
impl ScheduledState {
    pub(crate) fn cancel_all(&self) {
        for job in self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .values()
        {
            let mut job = job.lock().unwrap_or_else(PoisonError::into_inner);
            job.cancel.cancel();
            job.view.config.enabled = false;
        }
    }
}
/// Enable a finite interval job only after live profile/sharing/telemetry validation.
#[tauri::command]
#[specta::specta]
pub(crate) async fn investigation_schedule(
    app: AppHandle,
    mut input: ScheduleRequest,
) -> Result<(), IpcError> {
    input.request.validate()?;
    input.config.limits()?;
    input.request.monitoring = true;
    if input.request.provider != input.config.provider || input.request.model != input.config.model
    {
        return Err(IpcError::invalid(
            "Schedule provider/model must match the preview",
        ));
    }
    let executor = context::executor(&app, &input.request)?;
    context::prepare(&executor, &input.request).await?;
    let state = app.state::<ScheduledState>();
    let mut jobs = state.0.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(job) = jobs.remove(&input.request.id) {
        job.lock()
            .unwrap_or_else(PoisonError::into_inner)
            .cancel
            .cancel();
    }
    if !input.config.enabled {
        return Ok(());
    }
    if jobs.len() >= 4 {
        return Err(IpcError::invalid("At most four investigation schedules"));
    }
    let job = Arc::new(Mutex::new(Job {
        view: ScheduledJob {
            id: input.request.id.clone(),
            connection: input.request.connection.clone(),
            database: input.request.database.clone(),
            config: input.config.clone(),
            running: false,
            failure: None,
            results: VecDeque::new(),
        },
        cancel: Cancellation::default(),
    }));
    jobs.insert(input.request.id.clone(), job.clone());
    tauri::async_runtime::spawn(run::repeat(app.clone(), input, job));
    Ok(())
}
/// Cancel removes only the selected job; always available while locked or disconnected.
#[expect(clippy::needless_pass_by_value, reason = "Tauri owns inputs")]
#[tauri::command]
#[specta::specta]
pub(crate) fn investigation_schedule_cancel(app: AppHandle, id: String) {
    if let Some(job) = app
        .state::<ScheduledState>()
        .0
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(&id)
    {
        job.lock()
            .unwrap_or_else(PoisonError::into_inner)
            .cancel
            .cancel();
    }
}
/// Results remain in app memory only; locking hides all content.
#[expect(clippy::needless_pass_by_value, reason = "Tauri owns inputs")]
#[tauri::command]
#[specta::specta]
pub(crate) fn investigation_schedules(app: AppHandle) -> Result<Vec<ScheduledJob>, IpcError> {
    app.state::<crate::security::Security>()
        .admit("investigation_schedules")?;
    Ok(app
        .state::<ScheduledState>()
        .0
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .values()
        .map(|job| {
            job.lock()
                .unwrap_or_else(PoisonError::into_inner)
                .view
                .clone()
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lock_disconnect_disables_jobs_and_cancels_the_active_generation() {
        let config = InvestigationSchedule {
            enabled: true,
            provider: "fixture".into(),
            model: "fixture".into(),
            interval_secs: 300,
            max_runs: 1,
            steps: 4,
            tokens: 10000,
            timeout_secs: 30,
            max_cost_micros: 10000,
            price_per_million_micros: 1000000,
            tools: vec!["read_health_snapshot".into()],
        };
        let job = Arc::new(Mutex::new(Job {
            view: ScheduledJob {
                id: "fixture".into(),
                connection: "connection".into(),
                database: "db".into(),
                config,
                running: true,
                failure: None,
                results: VecDeque::new(),
            },
            cancel: Cancellation::default(),
        }));
        let state = ScheduledState::default();
        state
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert("fixture".into(), job.clone());
        state.cancel_all();
        let job = job.lock().unwrap_or_else(PoisonError::into_inner);
        assert!(!job.view.config.enabled);
        use std::future::Future;
        let mut future = std::pin::pin!(job.cancel.cancelled());
        assert!(
            future
                .as_mut()
                .poll(&mut std::task::Context::from_waker(std::task::Waker::noop()))
                .is_ready()
        );
    }
}

/// Retained payload-free usage/outcome metadata; existing audit purge clears it.
#[expect(clippy::needless_pass_by_value, reason = "Tauri owns inputs")]
#[tauri::command]
#[specta::specta]
pub(crate) fn investigation_audit(app: AppHandle) -> Result<Vec<InvestigationAudit>, IpcError> {
    app.state::<crate::security::Security>()
        .admit("investigation_audit")?;
    app.state::<openrows_ai::sharing::SharingStore>()
        .investigations()
        .map_err(|_| openrows_ai::AiError::Audit.into())
}

// SPDX-License-Identifier: Apache-2.0
//! Explicit user actions on scheduled investigations.
use super::{
    Job, JobState, ScheduleRequest, ScheduledJob, ScheduledState, guard, pause_reason, run,
};
use crate::{assistant::context, connections::IpcError};
use openrows_ai::sharing::InvestigationAudit;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};

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
    let mut jobs = state.jobs();
    if let Some(job) = jobs.remove(&input.request.id) {
        guard(&job).cancel.cancel();
    }
    if !input.config.enabled {
        return Ok(());
    }
    // Ended jobs stay visible until removed, but never crowd out new ones.
    jobs.retain(|_, job| guard(job).active());
    if jobs.len() >= 4 {
        return Err(IpcError::invalid("At most four investigation schedules"));
    }
    let job = Arc::new(Mutex::new(Job::new(&input.request, input.config.clone())));
    jobs.insert(input.request.id.clone(), job.clone());
    tauri::async_runtime::spawn(run::repeat(app.clone(), input, job));
    Ok(())
}
/// Cancel removes only the selected job; always available while locked or disconnected.
#[expect(clippy::needless_pass_by_value, reason = "Tauri owns inputs")]
#[tauri::command]
#[specta::specta]
pub(crate) fn investigation_schedule_cancel(app: AppHandle, id: String) -> Result<(), IpcError> {
    openrows_ai::validate_id(&id)?;
    if let Some(job) = app.state::<ScheduledState>().jobs().remove(&id) {
        let job = guard(&job);
        job.cancel.cancel();
        job.interrupt();
    }
    Ok(())
}
/// Pause stops the in-flight run and keeps the job; Resume lets the next tick run.
#[expect(clippy::needless_pass_by_value, reason = "Tauri owns inputs")]
#[tauri::command]
#[specta::specta]
pub(crate) fn investigation_schedule_pause(
    app: AppHandle,
    id: String,
    paused: bool,
) -> Result<(), IpcError> {
    openrows_ai::validate_id(&id)?;
    let job = app.state::<ScheduledState>().job(&id)?;
    let mut job = guard(&job);
    if !job.active() {
        return Err(IpcError::invalid("This schedule has ended"));
    }
    job.paused = paused;
    if paused {
        job.interrupt();
    }
    job.view.pause = pause_reason(&app, &job);
    if job.view.state != JobState::Running {
        job.view.state = state_for(&job);
    }
    Ok(())
}
/// Start the next run now instead of waiting for the interval; pauses still apply.
#[expect(clippy::needless_pass_by_value, reason = "Tauri owns inputs")]
#[tauri::command]
#[specta::specta]
pub(crate) fn investigation_schedule_run(app: AppHandle, id: String) -> Result<(), IpcError> {
    app.state::<crate::security::Security>()
        .admit("investigation_schedule_run")?;
    openrows_ai::validate_id(&id)?;
    let job = app.state::<ScheduledState>().job(&id)?;
    let job = guard(&job);
    if let Some(reason) = pause_reason(&app, &job) {
        return Err(IpcError::invalid(match reason {
            super::PauseReason::User => "Resume the schedule first",
            super::PauseReason::Locked => "Unlock the app first",
            super::PauseReason::AiOff => "Turn AI on in Settings first",
        }));
    }
    if !job.active() || job.view.state == JobState::Running {
        return Err(IpcError::invalid("This schedule cannot run now"));
    }
    job.wake.notify_one();
    Ok(())
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
        .jobs()
        .values()
        .map(|job| {
            let mut job = guard(job);
            if job.active() && job.view.state != JobState::Running {
                job.view.pause = pause_reason(&app, &job);
                job.view.state = state_for(&job);
            }
            job.view.clone()
        })
        .collect())
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
fn state_for(job: &Job) -> JobState {
    if job.view.pause.is_some() {
        JobState::Paused
    } else {
        JobState::Waiting
    }
}

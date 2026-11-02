// SPDX-License-Identifier: Apache-2.0
//! Session opt-in recurring jobs. Lock or AI off pauses them; only cancel removes one.
mod commands;
mod run;
#[cfg(test)]
mod tests;
use super::AssistantRequest;
use crate::connections::{IpcError, IpcErrorKind};
pub(crate) use commands::*;
use openrows_ai::{Cancellation, scheduled::InvestigationSchedule, sharing::InvestigationAudit};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};
use tauri::{AppHandle, Manager};

#[derive(Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScheduledResult {
    audit: InvestigationAudit,
    events: Vec<openrows_ai::agent::AgentEvent>,
}
/// Lifecycle shown to the user; only `Waiting` and `Running` can reach a provider.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum JobState {
    /// Enabled; the next run follows the interval or Run now.
    Waiting,
    /// A bounded run is in flight.
    Running,
    /// No run starts until the pause reason clears.
    Paused,
    /// All configured runs completed.
    Finished,
    /// A run failed or was refused; a fresh opt-in is required.
    Stopped,
}
/// Why a job is not running; lock and AI off clear by themselves.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum PauseReason {
    /// Paused by the user until Resume.
    User,
    /// The app is locked.
    Locked,
    /// AI is turned off in Settings.
    AiOff,
}
#[derive(Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScheduledJob {
    id: String,
    connection: String,
    database: String,
    config: InvestigationSchedule,
    state: JobState,
    pause: Option<PauseReason>,
    runs: u32,
    failure: Option<String>,
    results: VecDeque<ScheduledResult>,
}
struct Job {
    view: ScheduledJob,
    /// Removes the job and ends its loop.
    cancel: Cancellation,
    /// The in-flight run only; pausing cancels it but keeps the job.
    run: Option<Cancellation>,
    /// Run now.
    wake: Arc<tokio::sync::Notify>,
    /// Pause requested by the user.
    paused: bool,
}
impl Job {
    fn new(request: &AssistantRequest, config: InvestigationSchedule) -> Self {
        Self {
            view: ScheduledJob {
                id: request.id.clone(),
                connection: request.connection.clone(),
                database: request.database.clone(),
                config,
                state: JobState::Waiting,
                pause: None,
                runs: 0,
                failure: None,
                results: VecDeque::new(),
            },
            cancel: Cancellation::default(),
            run: None,
            wake: Arc::default(),
            paused: false,
        }
    }
    fn interrupt(&self) {
        if let Some(run) = &self.run {
            run.cancel();
        }
    }
    fn active(&self) -> bool {
        matches!(
            self.view.state,
            JobState::Waiting | JobState::Running | JobState::Paused
        )
    }
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
    /// Lock, AI off, key changes and closed sessions stop in-flight runs.
    /// Jobs stay and run again once nothing pauses them.
    pub(crate) fn interrupt_all(&self) {
        for job in self.jobs().values() {
            guard(job).interrupt();
        }
    }
    fn jobs(&self) -> MutexGuard<'_, BTreeMap<String, Arc<Mutex<Job>>>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
    fn job(&self, id: &str) -> Result<Arc<Mutex<Job>>, IpcError> {
        self.jobs()
            .get(id)
            .cloned()
            .ok_or_else(|| IpcError::new(IpcErrorKind::NotFound, "Schedule not found"))
    }
}
fn guard(job: &Mutex<Job>) -> MutexGuard<'_, Job> {
    job.lock().unwrap_or_else(PoisonError::into_inner)
}
/// The reason no run may start now, checked before every run.
fn pause_reason(app: &AppHandle, job: &Job) -> Option<PauseReason> {
    if job.paused {
        Some(PauseReason::User)
    } else if app
        .state::<crate::security::Security>()
        .admit("investigation_schedule")
        .is_err()
    {
        Some(PauseReason::Locked)
    } else if !app.state::<crate::ai::AiState>().control.enabled() {
        Some(PauseReason::AiOff)
    } else {
        None
    }
}

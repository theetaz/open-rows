// SPDX-License-Identifier: Apache-2.0
//! Bounded automatic runs reuse the same gate and read/propose executor as user turns.
use super::{Job, JobState, ScheduleRequest, ScheduledResult, guard, pause_reason};
use crate::{assistant::context, connections::IpcError};
use openrows_ai::{
    AiError, Cancellation, RequestOptions,
    agent::{
        AgentError, AgentEvent, AgentRequest, AgentTools, ToolCall, ToolDefinition, ToolReply,
        run_agent,
    },
    sharing::{InvestigationAudit, SharingStore},
};
use openrows_driver_api::BoxFuture;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tauri::{AppHandle, Manager};
struct SelectedTools {
    executor: openrows_mcp::ToolExecutor,
    allowed: Vec<String>,
}
impl AgentTools for SelectedTools {
    fn definitions(&self) -> Vec<ToolDefinition> {
        self.executor
            .definitions()
            .into_iter()
            .filter(|tool| self.allowed.contains(&tool.name))
            .collect()
    }
    fn call<'a>(&'a self, call: &'a ToolCall) -> BoxFuture<'a, Result<ToolReply, AgentError>> {
        Box::pin(async move {
            if !self.allowed.contains(&call.name) {
                return Err(AgentError::Refused);
            }
            self.executor.call(call).await
        })
    }
}
pub(super) async fn repeat(app: AppHandle, input: ScheduleRequest, job: Arc<Mutex<Job>>) {
    let (cancel, wake) = {
        let job = guard(&job);
        (job.cancel.clone(), job.wake.clone())
    };
    let period = Duration::from_secs(u64::from(input.config.interval_secs));
    let mut timer = tokio::time::interval_at(tokio::time::Instant::now() + period, period);
    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            biased;
            () = cancel.cancelled() => return,
            _ = timer.tick() => {},
            () = wake.notified() => {},
        }
        let Some(run) = begin(&app, &job) else {
            continue;
        };
        let result = execute(&app, &input, run.clone()).await;
        let mut current = guard(&job);
        current.view.pause = pause_reason(&app, &current);
        if record(&mut current, run.is_cancelled(), result) {
            return;
        }
    }
}
/// Start a run only when nothing pauses the job.
fn begin(app: &AppHandle, job: &Mutex<Job>) -> Option<Cancellation> {
    let mut job = guard(job);
    job.view.pause = pause_reason(app, &job);
    if job.view.pause.is_some() {
        job.view.state = JobState::Paused;
        return None;
    }
    let run = Cancellation::default();
    job.run = Some(run.clone());
    job.view.state = JobState::Running;
    Some(run)
}
/// Record a run; returns whether the job has ended. A run interrupted by a
/// pause, lock or AI off neither counts nor stops the job.
pub(super) fn record(
    job: &mut Job,
    cancelled: bool,
    result: Result<ScheduledResult, IpcError>,
) -> bool {
    job.run = None;
    let interrupted = cancelled || job.view.pause.is_some();
    let outcome = match result {
        Ok(result) => {
            let outcome = result.audit.outcome.clone();
            job.view.results.push_back(result);
            while job.view.results.len() > 10 {
                job.view.results.pop_front();
            }
            Ok(outcome)
        }
        Err(error) => Err(error.to_string()),
    };
    job.view.state = match outcome {
        _ if interrupted && job.view.pause.is_some() => JobState::Paused,
        _ if interrupted => JobState::Waiting,
        Ok(outcome) if outcome == "completed" => {
            job.view.runs += 1;
            if job.view.runs >= job.view.config.max_runs {
                JobState::Finished
            } else {
                JobState::Waiting
            }
        }
        Ok(outcome) => {
            job.view.runs += 1;
            job.view.failure = Some(format!("The last run was {outcome}"));
            JobState::Stopped
        }
        Err(error) => {
            job.view.failure = Some(error);
            JobState::Stopped
        }
    };
    !job.active()
}
async fn execute(
    app: &AppHandle,
    input: &ScheduleRequest,
    cancel: Cancellation,
) -> Result<ScheduledResult, IpcError> {
    let limits = input.config.limits()?;
    let executor = context::executor(app, &input.request)?;
    let store = app.state::<SharingStore>();
    let mut audit = InvestigationAudit {
        time: crate::queries::history::now_ms().to_string(),
        connection: input.request.connection.clone(),
        provider: input.config.provider.clone(),
        model: input.config.model.clone(),
        tools: vec![],
        evidence: vec![],
        tokens: limits.tokens,
        cost_micros: input.config.cost_micros(limits.tokens),
        outcome: "running".into(),
    };
    store
        .record_investigation(&audit)
        .map_err(|_| AiError::Audit)?;
    let options = RequestOptions {
        cancel,
        timeout: limits.timeout,
    };
    let tools = SelectedTools {
        executor,
        allowed: input.config.tools.clone(),
    };
    let mut events = Vec::new();
    let generation = Box::pin(turn(app, input, (&tools, options.clone()), &mut events));
    let work = tools
        .executor
        .control
        .run(&options, async { Ok(generation.await) })
        .await;
    account(&input.config, (&events, &tools.executor), &work, &mut audit);
    store
        .record_investigation(&audit)
        .map_err(|_| AiError::Audit)?;
    Ok(ScheduledResult { audit, events })
}
fn account(
    config: &openrows_ai::scheduled::InvestigationSchedule,
    (events, executor): (&[AgentEvent], &openrows_mcp::ToolExecutor),
    work: &Result<Result<(), IpcError>, AiError>,
    audit: &mut InvestigationAudit,
) {
    audit.tokens = events
        .iter()
        .find_map(|event| {
            if let AgentEvent::Done { tokens } = event {
                Some(*tokens)
            } else {
                None
            }
        })
        .unwrap_or(audit.tokens);
    audit.evidence = executor.ledger.ids(&audit.connection);
    audit.cost_micros = config.cost_micros(audit.tokens);
    audit.tools = events
        .iter()
        .filter_map(|event| {
            if let AgentEvent::Trace { trace } = event {
                Some(trace.tool.clone())
            } else {
                None
            }
        })
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    audit.outcome = match work {
        Ok(Ok(())) => "completed",
        Err(AiError::Cancelled | AiError::Disabled) => "cancelled",
        Ok(Err(error)) if error.kind == crate::connections::IpcErrorKind::Cancelled => "cancelled",
        Ok(Err(error)) if error.kind == crate::connections::IpcErrorKind::Unsupported => "refused",
        _ => "failed",
    }
    .into();
}
async fn turn(
    app: &AppHandle,
    input: &ScheduleRequest,
    (tools, options): (&SelectedTools, RequestOptions),
    events: &mut Vec<AgentEvent>,
) -> Result<(), IpcError> {
    let context = context::prepare(&tools.executor, &input.request).await?;
    let provider =
        crate::ai::generation_provider(app, &input.config.provider, &input.config.model).await?;
    let request = AgentRequest {
        model: input.config.model.clone(),
        context,
        limits: input.config.limits()?,
        options,
    };
    let (send, mut receive) = tokio::sync::mpsc::channel(16);
    let work = run_agent(&provider, tools, &request, send);
    tokio::pin!(work);
    loop {
        tokio::select! {
            biased;
            event=receive.recv()=>if let Some(event)=event { retain(events, event)?; } else { break; },
            result=&mut work=>{ result?; while let Ok(event)=receive.try_recv() { retain(events,event)?; } return Ok(()); }
        }
    }
    work.await.map_err(Into::into)
}
pub(super) fn retain(events: &mut Vec<AgentEvent>, event: AgentEvent) -> Result<(), IpcError> {
    let before = serde_json::to_vec(&events)
        .map_err(|_| AiError::InvalidResponse)?
        .len();
    let addition = serde_json::to_vec(&event)
        .map_err(|_| AiError::InvalidResponse)?
        .len();
    if events.len() >= 256 || before + addition + usize::from(!events.is_empty()) > 131_072 {
        return Err(AiError::InvalidResponse.into());
    }
    if let AgentEvent::Text { delta } = &event
        && let Some(AgentEvent::Text { delta: previous }) = events.last_mut()
    {
        previous.push_str(delta);
    } else {
        events.push(event);
    }
    Ok(())
}

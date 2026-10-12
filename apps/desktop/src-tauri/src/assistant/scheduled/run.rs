// SPDX-License-Identifier: Apache-2.0
//! Bounded automatic runs reuse the same gate and read/propose executor as user turns.
use super::{Job, ScheduleRequest, ScheduledResult};
use crate::{assistant::context, connections::IpcError};
use openrows_ai::{
    AiError, RequestOptions,
    agent::{
        AgentError, AgentEvent, AgentRequest, AgentTools, ToolCall, ToolDefinition, ToolReply,
        run_agent,
    },
    sharing::{InvestigationAudit, SharingStore},
};
use openrows_driver_api::BoxFuture;
use std::{
    sync::{Arc, Mutex, PoisonError},
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
    let cancel = job
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .cancel
        .clone();
    let mut timer = tokio::time::interval_at(
        tokio::time::Instant::now() + Duration::from_secs(u64::from(input.config.interval_secs)),
        Duration::from_secs(u64::from(input.config.interval_secs)),
    );
    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    for _ in 0..input.config.max_runs {
        tokio::select! { biased; () = cancel.cancelled() => break, _ = timer.tick() => {} }
        job.lock()
            .unwrap_or_else(PoisonError::into_inner)
            .view
            .running = true;
        let result = execute(&app, &input, cancel.clone()).await;
        let failed = result
            .as_ref()
            .map_or(true, |result| result.audit.outcome != "completed");
        let mut current = job.lock().unwrap_or_else(PoisonError::into_inner);
        current.view.running = false;
        match result {
            Ok(result) => current.view.results.push_back(result),
            Err(error) => current.view.failure = Some(error.to_string()),
        }
        while current.view.results.len() > 10 {
            current.view.results.pop_front();
        }
        if failed {
            break;
        }
    }
    job.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .view
        .config
        .enabled = false;
}
async fn execute(
    app: &AppHandle,
    input: &ScheduleRequest,
    cancel: openrows_ai::Cancellation,
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
    let work = tools
        .executor
        .control
        .run(&options, async {
            Ok(turn(app, input, (&tools, options.clone()), &mut events).await)
        })
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
fn retain(events: &mut Vec<AgentEvent>, event: AgentEvent) -> Result<(), IpcError> {
    let before = serde_json::to_vec(&events)
        .map_err(|_| AiError::InvalidResponse)?
        .len();
    let addition = serde_json::to_vec(&event)
        .map_err(|_| AiError::InvalidResponse)?
        .len();
    if events.len() >= 256 || before + addition > 131072 {
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retention_refuses_oversized_events_before_mutating_existing_results() -> Result<(), IpcError>
    {
        let mut events = vec![];
        retain(
            &mut events,
            AgentEvent::Text {
                delta: "safe".into(),
            },
        )?;
        assert!(
            retain(
                &mut events,
                AgentEvent::Text {
                    delta: "x".repeat(131072)
                }
            )
            .is_err()
        );
        assert!(matches!(&events[..], [AgentEvent::Text { delta }] if delta == "safe"));
        let mut events = vec![AgentEvent::Done { tokens: 0 }; 256];
        assert!(retain(&mut events, AgentEvent::Done { tokens: 0 }).is_err());
        assert_eq!(events.len(), 256);
        Ok(())
    }
}

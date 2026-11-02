// SPDX-License-Identifier: Apache-2.0
use super::{
    Job, JobState, PauseReason, ScheduledResult, ScheduledState, guard,
    run::{record, retain},
};
use crate::connections::IpcError;
use openrows_ai::{
    AiError, Cancellation, agent::AgentEvent, scheduled::InvestigationSchedule,
    sharing::InvestigationAudit,
};
use std::sync::{Arc, Mutex};

fn config(max_runs: u32) -> InvestigationSchedule {
    InvestigationSchedule {
        enabled: true,
        provider: "fixture".into(),
        model: "fixture".into(),
        interval_secs: 300,
        max_runs,
        steps: 4,
        tokens: 10000,
        timeout_secs: 30,
        max_cost_micros: 10000,
        price_per_million_micros: 1_000_000,
        tools: vec!["read_health_snapshot".into()],
    }
}
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn job(max_runs: u32) -> Result<Job, serde_json::Error> {
    let mut job = Job::new(
        &serde_json::from_value(serde_json::json!({
            "id": "fixture", "provider": "fixture", "model": "fixture",
            "connection": "connection", "database": "db", "schema": "public",
            "question": {"question": "Investigate", "history": [], "errorCode": null},
            "query": null, "plan": []
        }))?,
        config(max_runs),
    );
    job.run = Some(Cancellation::default());
    Ok(job)
}
fn result(outcome: &str) -> ScheduledResult {
    ScheduledResult {
        audit: InvestigationAudit {
            time: "1".into(),
            connection: "connection".into(),
            provider: "fixture".into(),
            model: "fixture".into(),
            tools: vec![],
            evidence: vec![],
            tokens: 10,
            cost_micros: 10,
            outcome: outcome.into(),
        },
        events: vec![],
    }
}

#[test]
fn schedule_commands_are_registered_and_allowed() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = include_str!("../../../build.rs");
    let capability: serde_json::Value =
        serde_json::from_str(include_str!("../../../capabilities/main.json"))?;
    let permissions = capability["permissions"]
        .as_array()
        .ok_or("Missing permissions")?;
    for command in [
        "investigation_schedule",
        "investigation_schedule_cancel",
        "investigation_schedule_pause",
        "investigation_schedule_run",
        "investigation_schedules",
        "investigation_audit",
    ] {
        assert!(manifest.contains(&format!("\"{command}\"")));
        let permission = format!("allow-{}", command.replace('_', "-"));
        assert!(
            permissions
                .iter()
                .any(|item| item.as_str() == Some(&permission))
        );
    }
    Ok(())
}
#[test]
fn lock_or_ai_off_interrupts_the_run_but_keeps_the_job() -> TestResult {
    let job = Arc::new(Mutex::new(job(2)?));
    let run = guard(&job).run.clone().unwrap_or_default();
    let state = ScheduledState::default();
    state.jobs().insert("fixture".into(), job.clone());
    state.interrupt_all();
    assert!(run.is_cancelled());
    let job = guard(&job);
    assert!(job.active());
    assert!(!job.cancel.is_cancelled());
    assert!(job.view.config.enabled);
    Ok(())
}
#[test]
fn an_interrupted_run_neither_counts_nor_stops_the_job() -> TestResult {
    let mut job = job(1)?;
    job.view.pause = Some(PauseReason::Locked);
    assert!(!record(&mut job, true, Ok(result("cancelled"))));
    assert_eq!(job.view.state, JobState::Paused);
    assert_eq!(job.view.runs, 0);
    assert!(job.view.failure.is_none());
    assert!(job.run.is_none());
    job.view.pause = None;
    assert!(!record(&mut job, true, Err(AiError::Disabled.into())));
    assert_eq!(job.view.state, JobState::Waiting);
    Ok(())
}
#[test]
fn completed_runs_count_until_the_limit_and_failures_stop_the_job() -> TestResult {
    let mut job = job(2)?;
    assert!(!record(&mut job, false, Ok(result("completed"))));
    assert_eq!((job.view.state, job.view.runs), (JobState::Waiting, 1));
    assert!(record(&mut job, false, Ok(result("completed"))));
    assert_eq!((job.view.state, job.view.runs), (JobState::Finished, 2));
    let mut job = self::job(5)?;
    assert!(record(&mut job, false, Ok(result("refused"))));
    assert_eq!(job.view.state, JobState::Stopped);
    assert_eq!(
        job.view.failure.as_deref(),
        Some("The last run was refused")
    );
    assert_eq!(job.view.results.len(), 1);
    Ok(())
}
#[test]
fn retention_counts_array_separators_before_accepting_an_event() -> Result<(), IpcError> {
    let mut events = vec![AgentEvent::Done { tokens: 0 }];
    let before = serde_json::to_vec(&events)
        .map_err(|_| AiError::InvalidResponse)?
        .len();
    let empty = AgentEvent::Proposal { sql: String::new() };
    let overhead = serde_json::to_vec(&empty)
        .map_err(|_| AiError::InvalidResponse)?
        .len();
    let event = AgentEvent::Proposal {
        sql: "x".repeat(131_072 - before - overhead),
    };
    assert!(retain(&mut events, event).is_err());
    assert_eq!(events.len(), 1);
    Ok(())
}
#[test]
fn retention_refuses_oversized_events_before_mutating_existing_results() -> Result<(), IpcError> {
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
                delta: "x".repeat(131_072)
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

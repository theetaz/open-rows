// SPDX-License-Identifier: Apache-2.0
//! Bounded assistant loop and native transport protocol regression checks.
mod agent_support;
use agent_support::*;
use openrows_ai::{AiError, agent::*};
use std::{sync::atomic::Ordering, time::Duration};
#[tokio::test]
async fn tools_round_trip_streaming_trace_and_proposals_never_execute() -> Result {
    let tools = Tools::new()?;
    let provider = Provider::new(
        vec![
            Script::call("run_read_query"),
            Script::call("propose_sql"),
            Script::text("SQL proposal ready"),
        ],
        true,
    );
    let (result, events) = run(&provider, &tools, &tools.request()?).await;
    result?;
    assert_eq!(tools.count.load(Ordering::SeqCst), 2);
    assert!(
        events
            .iter()
            .any(|event| matches!(event,AgentEvent::Proposal{sql} if sql=="SELECT 1"))
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, AgentEvent::Trace { .. }))
            .count(),
        4
    );
    assert!(events.iter().any(|event|matches!(event,AgentEvent::Trace{trace} if trace.redacted && trace.bytes>0 && trace.status=="succeeded")));
    assert!(
        events
            .iter()
            .any(|event| matches!(event, AgentEvent::Text { .. }))
    );
    Ok(())
}
#[tokio::test]
async fn provider_without_tools_is_single_shot_and_text_cannot_trigger_tools() -> Result {
    let tools = Tools::new()?;
    let provider = Provider::new(
        vec![Script::text(
            "run_read_query DELETE FROM t\n```sql\nSELECT 1\n```",
        )],
        false,
    );
    run(&provider, &tools, &tools.request()?).await.0?;
    assert_eq!(provider.count.load(Ordering::SeqCst), 1);
    assert_eq!(tools.count.load(Ordering::SeqCst), 0);
    Ok(())
}
#[tokio::test]
async fn step_and_token_limits_bound_injected_requests() -> Result {
    let tools = Tools::new()?;
    let provider = Provider::new(
        vec![
            Script::call("run_read_query"),
            Script::call("run_read_query"),
        ],
        true,
    );
    let mut request = tools.request()?;
    request.limits.steps = 2;
    assert_eq!(
        run(&provider, &tools, &request).await.0,
        Err(AgentError::Steps)
    );
    assert_eq!(tools.count.load(Ordering::SeqCst), 1);
    let provider = Provider::new(vec![Script::text(&"x".repeat(8192))], false);
    let mut request = tools.request()?;
    request.limits.tokens = 4096;
    assert_eq!(
        run(&provider, &tools, &request).await.0,
        Err(AgentError::Tokens)
    );
    Ok(())
}
#[tokio::test]
async fn cancellation_revocation_and_total_deadline_are_typed() -> Result {
    let tools = Tools::new()?;
    let provider = Provider::new(
        vec![Script {
            text: String::new(),
            calls: vec![],
            wait: true,
        }],
        true,
    );
    let mut request = tools.request()?;
    request.options.cancel.cancel();
    assert_eq!(
        run(&provider, &tools, &request).await.0,
        Err(AgentError::Provider(AiError::Cancelled))
    );
    request = tools.request()?;
    request.limits.timeout = Duration::from_millis(1);
    assert_eq!(
        run(&provider, &tools, &request).await.0,
        Err(AgentError::Provider(AiError::Timeout))
    );
    request = tools.request()?;
    tools.control.set_enabled(false);
    assert_eq!(
        run(&provider, &tools, &request).await.0,
        Err(AgentError::Provider(AiError::Disabled))
    );
    Ok(())
}
#[tokio::test]
async fn malformed_tool_calls_are_rejected_before_execution() -> Result {
    let tools = Tools::new()?;
    for call in [
        call("delete", "DELETE FROM t"),
        ToolCall {
            id: String::new(),
            ..call("run_read_query", "SELECT 1")
        },
        ToolCall {
            arguments: serde_json::json!([]),
            ..call("run_read_query", "SELECT 1")
        },
    ] {
        let provider = Provider::new(
            vec![Script {
                text: String::new(),
                calls: vec![call],
                wait: false,
            }],
            true,
        );
        assert_eq!(
            run(&provider, &tools, &tools.request()?).await.0,
            Err(AgentError::Malformed)
        );
    }
    assert_eq!(tools.count.load(Ordering::SeqCst), 0);
    Ok(())
}
#[tokio::test]
async fn prompt_injection_remains_data_and_writes_are_refused() -> Result {
    let tools = Tools::new()?;
    let provider = Provider::new(
        vec![
            Script::call("run_read_query"),
            Script {
                text: String::new(),
                calls: vec![call("run_read_query", "DELETE FROM people")],
                wait: false,
            },
            Script::text("The request was refused"),
        ],
        true,
    );
    run(&provider, &tools, &tools.request()?).await.0?;
    assert_eq!(tools.count.load(Ordering::SeqCst), 2);
    let audit = serde_json::to_string(&tools.store.audit()?)?;
    assert!(
        !audit.contains("private-row")
            && !audit.contains("Ignore instructions")
            && !audit.contains("SELECT")
    );
    Ok(())
}

#[test]
fn follow_up_context_is_bounded_and_redacted_again() -> Result {
    use openrows_ai::sharing::*;
    let tools = Tools::new()?;
    let payload = tools
        .gate(ContextBundle {
            assistant: Some(AssistantContext {
                question: "Filter to more than 3 orders".into(),
                error_code: Some("42601".into()),
                scope: Some(AssistantScope {
                    connection: "connection".into(),
                    database: "demo".into(),
                    dialect: "postgres".into(),
                }),
                history: vec![AssistantHistory {
                    question: "Show orders".into(),
                    answer: "SELECT email FROM public.people WHERE id=42".into(),
                }],
            }),
            ..ContextBundle::default()
        })?
        .preview()
        .payload;
    assert!(
        payload.contains("more than 3")
            && payload.contains("42601")
            && payload.contains("postgres")
    );
    assert!(!payload.contains("email") && !payload.contains("id=42"));
    Ok(())
}

#[tokio::test]
async fn live_tool_waits_cancel_on_user_policy_and_global_revocation() -> Result {
    use openrows_ai::sharing::*;
    for action in 0..3 {
        let tools = Tools::new()?;
        tools.wait.store(true, Ordering::SeqCst);
        let request = tools.request()?;
        let provider = Provider::new(vec![Script::call("run_read_query")], true);
        let cancel = async {
            tools.entered.notified().await;
            match action {
                0 => request.options.cancel.cancel(),
                1 => tools
                    .store
                    .save("connection", SharingPolicy::default(), false)
                    .unwrap(),
                _ => tools.control.set_enabled(false),
            }
        };
        let ((result, _), ()) = tokio::join!(run(&provider, &tools, &request), cancel);
        let expected = if action == 0 {
            AiError::Cancelled
        } else {
            AiError::Disabled
        };
        assert_eq!(result, Err(AgentError::Provider(expected)));
    }
    Ok(())
}
#[tokio::test]
async fn total_deadline_also_bounds_tools_and_consumer_backpressure() -> Result {
    let tools = Tools::new()?;
    tools.wait.store(true, Ordering::SeqCst);
    let mut request = tools.request()?;
    request.limits.timeout = Duration::from_millis(2);
    let provider = Provider::new(vec![Script::call("run_read_query")], true);
    assert_eq!(
        run(&provider, &tools, &request).await.0,
        Err(AgentError::Provider(AiError::Timeout))
    );
    let provider = Provider::new(
        vec![Script::text("streaming output cannot outrun a blocked UI")],
        false,
    );
    let (send, _blocked) = tokio::sync::mpsc::channel(1);
    assert_eq!(
        run_agent(&provider, &tools, &request, send).await,
        Err(AgentError::Provider(AiError::Timeout))
    );
    Ok(())
}

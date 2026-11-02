// SPDX-License-Identifier: Apache-2.0
//! Codex conversations through the shared agent loop against the fake app-server.
mod agent_support;
mod codex_support;
use agent_support::{Tools, run};
use codex_support::{FAKE, Fixture, Result, config, reaped};
use openrows_ai::{
    AiError, RequestOptions,
    agent::{AgentError, AgentEvent, AgentRequest, run_agent},
    codex::CodexProvider,
};
use serde_json::{Value, json};
use std::{sync::atomic::Ordering, time::Duration};

fn setup(scenario: &str) -> Result<(Fixture, Tools, CodexProvider)> {
    let fixture = Fixture::new(scenario)?;
    let mut tools = Tools::new()?;
    tools.config = config(FAKE);
    let provider = CodexProvider::new(
        fixture.bridge.clone(),
        tools.config.clone(),
        tools.control.clone(),
    );
    Ok((fixture, tools, provider))
}
fn text(events: &[AgentEvent]) -> String {
    events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::Text { delta } => Some(delta.as_str()),
            _ => None,
        })
        .collect()
}
#[tokio::test]
async fn a_turn_streams_text_in_an_ephemeral_read_only_thread() -> Result {
    let (fixture, tools, provider) = setup("chat")?;
    let request = tools.request()?;
    let (result, events) = run(&provider, &tools, &request).await;
    result?;
    assert_eq!(text(&events), "Hello world");
    assert!(matches!(events.last(), Some(AgentEvent::Done { .. })));
    let thread = fixture.recorded("thread")?;
    assert_eq!(thread["ephemeral"], true);
    assert_eq!(thread["sandbox"], "read-only");
    assert_eq!(thread["approvalPolicy"], "on-request");
    assert!(
        thread["developerInstructions"]
            .as_str()
            .is_some_and(|text| text.contains("untrusted data"))
    );
    let names: Vec<_> = thread["dynamicTools"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|tool| tool["name"].clone())
        .collect();
    assert_eq!(names, [json!("run_read_query"), json!("propose_sql")]);
    let turn = fixture.recorded("turn")?;
    assert_eq!(
        turn["sandboxPolicy"],
        json!({"type": "readOnly", "networkAccess": false})
    );
    // Exactly the gated, redacted preview is sent; nothing else.
    assert_eq!(turn["input"][0]["text"], request.context.preview().payload);
    assert_eq!(
        fixture.recorded("initialize")?["capabilities"]["experimentalApi"],
        true
    );
    Ok(())
}
#[tokio::test]
async fn tool_calls_return_only_gated_redacted_and_audited_results() -> Result {
    let (fixture, tools, provider) = setup("tool")?;
    let (result, events) = run(&provider, &tools, &tools.request()?).await;
    result?;
    assert_eq!(tools.count.load(Ordering::SeqCst), 1);
    assert_eq!(text(&events), "Checking. Done.");
    assert!(events.iter().any(
        |event| matches!(event, AgentEvent::Trace { trace } if trace.tool == "run_read_query")
    ));
    let reply = fixture.recorded("tool-result")?;
    assert_eq!(reply["id"], "tool-result");
    assert_eq!(reply["result"]["success"], true);
    let data = reply["result"]["contentItems"][0]["text"]
        .as_str()
        .unwrap_or_default();
    assert!(data.contains("‹redacted›") && !data.contains("private-row"));
    assert!(!tools.store.audit()?.is_empty());
    Ok(())
}
#[tokio::test]
async fn every_approval_and_unsupported_request_is_refused() -> Result {
    let (fixture, tools, provider) = setup("approval")?;
    let (result, events) = run(&provider, &tools, &tools.request()?).await;
    result?;
    assert_eq!(text(&events), "Declined.");
    let result = |name: &str| fixture.recorded(name).map(|reply| reply["result"].clone());
    assert_eq!(result("command-approval")?, json!({"decision": "decline"}));
    assert_eq!(result("file-approval")?, json!({"decision": "decline"}));
    assert_eq!(
        result("permissions")?,
        json!({"permissions": {}, "scope": "turn"})
    );
    assert_eq!(result("elicitation")?["action"], "decline");
    assert_eq!(fixture.recorded("user-input")?["error"]["code"], -32601);
    assert_eq!(tools.count.load(Ordering::SeqCst), 0);
    Ok(())
}
#[tokio::test]
async fn crashes_failures_and_oversized_output_are_typed_errors() -> Result {
    for (scenario, expected) in [
        ("crash", AiError::Network),
        ("failed", AiError::Quota),
        ("flood", AiError::InvalidResponse),
    ] {
        let (_fixture, tools, provider) = setup(scenario)?;
        let (result, _) = run(&provider, &tools, &tools.request()?).await;
        assert_eq!(result, Err(AgentError::Provider(expected)), "{scenario}");
    }
    Ok(())
}
#[tokio::test]
async fn off_permission_never_launches_the_cli() -> Result {
    let (fixture, tools, provider) = setup("chat")?;
    let request = tools.request()?;
    tools.control.set_enabled(false);
    let (result, _) = run(&provider, &tools, &request).await;
    assert_eq!(result, Err(AgentError::Provider(AiError::Disabled)));
    assert!(!fixture.launched());
    Ok(())
}
async fn stalled(
    options: RequestOptions,
    cancel: bool,
) -> Result<(Fixture, std::result::Result<(), AgentError>)> {
    let (fixture, tools, provider) = setup("hang")?;
    let mut request = tools.request()?;
    request.options = options;
    let handle = request.options.cancel.clone();
    let (send, mut receive) = tokio::sync::mpsc::channel(64);
    let work = run_agent(&provider, &tools, &request, send);
    tokio::pin!(work);
    let first = tokio::select! {
        event = receive.recv() => event,
        result = &mut work => return Ok((fixture, result)),
    };
    assert!(matches!(first, Some(AgentEvent::Text { ref delta }) if delta == "partial"));
    if cancel {
        handle.cancel();
    }
    let result = work.await;
    Ok((fixture, result))
}
fn pids(fixture: &Fixture) -> Result<Vec<u64>> {
    let pids = fixture.recorded("pids")?;
    Ok(pids
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_u64)
        .collect())
}
#[tokio::test]
async fn cancel_kills_and_reaps_the_whole_process_tree() -> Result {
    let options = RequestOptions {
        timeout: Duration::from_secs(30),
        ..RequestOptions::default()
    };
    let (fixture, result) = Box::pin(stalled(options, true)).await?;
    assert_eq!(result, Err(AgentError::Provider(AiError::Cancelled)));
    let pids = pids(&fixture)?;
    assert_eq!(pids.len(), 2);
    assert!(reaped(&pids), "CLI and grandchild must exit");
    Ok(())
}
#[tokio::test]
async fn the_deadline_kills_a_stalled_cli() -> Result {
    let options = RequestOptions {
        timeout: Duration::from_secs(3),
        ..RequestOptions::default()
    };
    let (fixture, result) = Box::pin(stalled(options, false)).await?;
    assert_eq!(result, Err(AgentError::Provider(AiError::Timeout)));
    assert!(reaped(&pids(&fixture)?));
    Ok(())
}
#[tokio::test]
async fn a_request_without_tools_offers_none() -> Result {
    let (fixture, tools, provider) = setup("chat")?;
    let provider = provider.with_tool_calling(false);
    let request: AgentRequest = tools.request()?;
    let (result, _) = run(&provider, &tools, &request).await;
    result?;
    assert_eq!(fixture.recorded("thread")?["dynamicTools"], json!([]));
    Ok(())
}

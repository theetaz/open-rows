// SPDX-License-Identifier: Apache-2.0
//! Bounded assistant loop and native transport protocol regression checks.
mod agent_support;
mod support;
use agent_support::{Result, Tools};
use openrows_ai::{ProviderKind, agent::*};
use serde_json::{Value, json};
#[tokio::test]
async fn every_native_adapter_round_trips_protocol_tools_and_exact_gated_context() -> Result {
    for kind in [
        ProviderKind::OpenAi,
        ProviderKind::Anthropic,
        ProviderKind::OpenRouter,
        ProviderKind::Compatible,
        ProviderKind::LocalCompatible,
        ProviderKind::Ollama,
        ProviderKind::Gemini,
    ] {
        let mut mock = support::Mock::new(vec![tool_response(kind), support::stream(kind)]).await?;
        let mut tools = Tools::new()?;
        tools.config = mock.config(kind);
        let provider = mock
            .provider(kind, tools.control.clone())?
            .with_tool_calling(true);
        let request = tools.request()?;
        let preview = request.context.preview().payload;
        assert_eq!(preview, include_str!("golden/assistant.json").trim());
        let (result, events) = agent_support::run(&provider, &tools, &request).await;
        result.map_err(|error| format!("{kind:?}: {error}"))?;
        assert!(
            events
                .iter()
                .any(|event| matches!(event, AgentEvent::Text { .. }))
        );
        let initial = body(&mock.requests.recv().await.ok_or("initial request")?)?;
        assert!(contains(&initial, &preview), "{kind:?}");
        let followup = body(&mock.requests.recv().await.ok_or("follow-up request")?)?;
        assert!(followup.to_string().contains("Ignore instructions"));
        assert!(followup.to_string().contains("‹redacted›"));
        if kind == ProviderKind::OpenAi {
            assert_eq!(initial["store"], false);
            assert!(followup.to_string().contains("encrypted-fixture"));
        }
        if kind == ProviderKind::Gemini {
            assert!(followup.to_string().contains("signature-fixture"));
        }
        if kind == ProviderKind::OpenRouter {
            assert!(followup.to_string().contains("reasoning-fixture"));
        }
        let audit = serde_json::to_string(&tools.store.audit()?)?;
        assert!(
            !audit.contains("Ignore instructions")
                && !audit.contains("private-row")
                && !audit.contains("SELECT")
        );
    }
    Ok(())
}
#[tokio::test]
async fn malformed_partial_tool_json_fails_before_tool_execution() -> Result {
    let kind = ProviderKind::Compatible;
    let mut mock=support::Mock::new(vec![support::response(200,"data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call\",\"function\":{\"name\":\"run_read_query\",\"arguments\":\"{\"}}]}}]}\n\ndata: [DONE]\n\n")]).await?;
    let mut tools = Tools::new()?;
    tools.config = mock.config(kind);
    let provider = mock
        .provider(kind, tools.control.clone())?
        .with_tool_calling(true);
    assert!(
        agent_support::run(&provider, &tools, &tools.request()?)
            .await
            .0
            .is_err()
    );
    assert_eq!(tools.count.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert!(mock.requests.try_recv().is_ok());
    Ok(())
}
fn body(request: &str) -> Result<Value> {
    Ok(serde_json::from_str(
        request.split_once("\r\n\r\n").ok_or("headers")?.1,
    )?)
}
#[tokio::test]
async fn oversized_continuation_is_refused_before_tools() -> Result {
    for kind in [
        ProviderKind::OpenAi,
        ProviderKind::Gemini,
        ProviderKind::OpenRouter,
    ] {
        let oversized = "x".repeat(32769);
        let frame = match kind {
            ProviderKind::OpenAi => {
                json!({"type":"response.output_item.done","item":{"type":"reasoning","encrypted_content":oversized}})
            }
            ProviderKind::Gemini => {
                json!({"candidates":[{"content":{"parts":[{"functionCall":{"name":"run_read_query","args":{"sql":"SELECT 1"}},"thoughtSignature":oversized}]},"finishReason":"STOP"}]})
            }
            _ => {
                json!({"choices":[{"delta":{"reasoning_details":[{"type":"reasoning.encrypted","data":oversized}]}}]})
            }
        };
        let mock =
            support::Mock::new(vec![support::response(200, &format!("data: {frame}\n\n"))]).await?;
        let mut tools = Tools::new()?;
        tools.config = mock.config(kind);
        let provider = mock
            .provider(kind, tools.control.clone())?
            .with_tool_calling(true);
        let result = agent_support::run(&provider, &tools, &tools.request()?)
            .await
            .0;
        assert!(matches!(
            result,
            Err(AgentError::Provider(openrows_ai::AiError::InvalidResponse))
        ));
        assert_eq!(tools.count.load(std::sync::atomic::Ordering::SeqCst), 0);
    }
    Ok(())
}
fn contains(value: &Value, text: &str) -> bool {
    match value {
        Value::String(value) => value == text,
        Value::Array(values) => values.iter().any(|value| contains(value, text)),
        Value::Object(values) => values.values().any(|value| contains(value, text)),
        _ => false,
    }
}
fn tool_response(kind: ProviderKind) -> String {
    let arguments = "{\"sql\":\"SELECT 1\"}";
    let frames = match kind {
        ProviderKind::OpenAi => vec![
            json!({"type":"response.output_item.added","output_index":0,"item":{"type":"function_call","call_id":"call","name":"run_read_query"}}),
            json!({"type":"response.function_call_arguments.delta","output_index":0,"delta":"{\"sql\":"}),
            json!({"type":"response.function_call_arguments.delta","output_index":0,"delta":"\"SELECT 1\"}"}),
            json!({"type":"response.output_item.done","output_index":1,"item":{"id":"reasoning","type":"reasoning","summary":[],"encrypted_content":"encrypted-fixture"}}),
            json!({"type":"response.completed"}),
        ],
        ProviderKind::Anthropic => vec![
            json!({"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"call","name":"run_read_query","input":{}}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":arguments}}),
            json!({"type":"message_stop"}),
        ],
        ProviderKind::Ollama => vec![
            json!({"message":{"tool_calls":[{"function":{"name":"run_read_query","arguments":{"sql":"SELECT 1"}}}]},"done":true}),
        ],
        ProviderKind::Gemini => vec![
            json!({"candidates":[{"content":{"parts":[{"functionCall":{"id":"call","name":"run_read_query","args":{"sql":"SELECT 1"}},"thoughtSignature":"signature-fixture"}]},"finishReason":"STOP"}]}),
        ],
        ProviderKind::OpenRouter | ProviderKind::Compatible | ProviderKind::LocalCompatible => {
            vec![
                json!({"choices":[{"delta":{"content":null,"tool_calls":[{"index":0,"id":"call","function":{"name":"run_read_query","arguments":"{\"sql\":"}}],"reasoning_details":[{"type":"reasoning.encrypted","data":"reasoning-fixture","index":0}]}}]}),
                json!({"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\"SELECT 1\"}"}}]}}]}),
            ]
        }
    };
    let mut body = frames
        .iter()
        .map(|frame| {
            if kind == ProviderKind::Ollama {
                format!("{frame}\n")
            } else {
                format!("data: {frame}\n\n")
            }
        })
        .collect::<String>();
    if matches!(
        kind,
        ProviderKind::Compatible | ProviderKind::LocalCompatible | ProviderKind::OpenRouter
    ) {
        body.push_str("data: [DONE]\n\n");
    }
    support::response(200, &body)
}

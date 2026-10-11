// SPDX-License-Identifier: Apache-2.0
//! Native protocol messages keep tool evidence in result roles, never instructions.
use crate::{
    AiError, ProviderKind,
    agent::{Message, TurnRequest, calls::INSTRUCTIONS},
};
use serde_json::{Value, json};
pub(crate) fn request(
    kind: ProviderKind,
    request: &TurnRequest<'_>,
) -> Result<(String, Value), AiError> {
    let mut messages = vec![json!({"role":"user","content":request.context.text()})];
    for message in request.messages {
        messages.push(chat_message(message, kind));
    }
    let tools: Vec<_> = request.tools.iter().map(|tool| json!({"type":"function","function":{"name":tool.name,"description":tool.description,"parameters":tool.schema}})).collect();
    let mut body = json!({"model":request.model,"messages":messages,"max_tokens":request.max_output,"stream":true});
    if !tools.is_empty() {
        body["tools"] = json!(tools);
    }
    match kind {
        ProviderKind::OpenAi => Ok(("responses".into(), responses(request))),
        ProviderKind::Anthropic => Ok(("messages".into(), anthropic(request))),
        ProviderKind::Gemini => gemini(request),
        ProviderKind::Ollama => {
            body.as_object_mut()
                .ok_or(AiError::InvalidConfig)?
                .remove("max_tokens");
            body["options"] = json!({"num_predict":request.max_output});
            body["messages"]
                .as_array_mut()
                .ok_or(AiError::InvalidConfig)?
                .insert(0, json!({"role":"system","content":INSTRUCTIONS}));
            Ok(("api/chat".into(), body))
        }
        ProviderKind::OpenRouter | ProviderKind::Compatible | ProviderKind::LocalCompatible => {
            body["messages"]
                .as_array_mut()
                .ok_or(AiError::InvalidConfig)?
                .insert(0, json!({"role":"system","content":INSTRUCTIONS}));
            Ok(("chat/completions".into(), body))
        }
    }
}
fn chat_message(message: &Message, kind: ProviderKind) -> Value {
    match message {
        Message::Assistant {
            text,
            calls,
            continuation,
        } => {
            let mut output = json!({"role":"assistant","content":text});
            if !continuation.chat.is_empty() {
                output["reasoning_details"] = json!(continuation.chat);
            }
            if !calls.is_empty() {
                output["tool_calls"] = json!(
                    calls
                        .iter()
                        .map(|call| super::continuation::chat_call(call, kind, continuation))
                        .collect::<Vec<_>>()
                );
            }
            output
        }
        Message::Tool { id, name, data, .. } => {
            json!({"role":"tool","tool_call_id":id,"tool_name":name,"content":data})
        }
    }
}
fn responses(request: &TurnRequest<'_>) -> Value {
    let mut input = vec![json!({"role":"user","content":request.context.text()})];
    for message in request.messages {
        match message {
            Message::Assistant {
                text,
                calls,
                continuation,
            } => {
                input.extend(continuation.responses.iter().cloned());
                if !text.is_empty() {
                    input.push(json!({"role":"assistant","content":text}));
                }
                input.extend(calls.iter().map(|call| json!({"type":"function_call","call_id":call.id,"name":call.name,"arguments":call.arguments.to_string()})));
            }
            Message::Tool { id, data, .. } => {
                input.push(json!({"type":"function_call_output","call_id":id,"output":data}));
            }
        }
    }
    let mut body = json!({"model":request.model,"instructions":INSTRUCTIONS,"input":input,"stream":true,"store":false,"max_output_tokens":request.max_output});
    if !request.tools.is_empty() {
        body["tools"] = json!(request.tools.iter().map(|tool| json!({"type":"function","name":tool.name,"description":tool.description,"parameters":tool.schema})).collect::<Vec<_>>());
    }
    body
}
fn anthropic(request: &TurnRequest<'_>) -> Value {
    let mut messages = vec![json!({"role":"user","content":request.context.text()})];
    for message in request.messages {
        match message {
            Message::Assistant { text, calls, .. } => {
                let mut content = Vec::new();
                if !text.is_empty() { content.push(json!({"type":"text","text":text})); }
                content.extend(calls.iter().map(|call| json!({"type":"tool_use","id":call.id,"name":call.name,"input":call.arguments})));
                messages.push(json!({"role":"assistant","content":content}));
            }
            Message::Tool { id, data, failed, .. } => messages.push(json!({"role":"user","content":[{"type":"tool_result","tool_use_id":id,"content":data,"is_error":failed}]})),
        }
    }
    let mut body = json!({"model":request.model,"system":INSTRUCTIONS,"messages":messages,"stream":true,"max_tokens":request.max_output});
    if !request.tools.is_empty() {
        body["tools"]=json!(request.tools.iter().map(|tool|json!({"name":tool.name,"description":tool.description,"input_schema":tool.schema})).collect::<Vec<_>>());
    }
    body
}
fn gemini(request: &TurnRequest<'_>) -> Result<(String, Value), AiError> {
    let model = request
        .model
        .strip_prefix("models/")
        .unwrap_or(request.model);
    if !model
        .bytes()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.'))
    {
        return Err(AiError::InvalidConfig);
    }
    let mut contents = vec![json!({"role":"user","parts":[{"text":request.context.text()}]})];
    let mut native_ids = std::collections::HashSet::new();
    for message in request.messages {
        match message {
            Message::Assistant {
                text,
                calls,
                continuation,
            } => {
                native_ids.extend(
                    continuation.gemini.iter().filter_map(|part| {
                        part.pointer("/functionCall/id").and_then(Value::as_str)
                    }),
                );
                let mut parts = Vec::new();
                if !text.is_empty() {
                    parts.push(json!({"text":text}));
                }
                if continuation.gemini.is_empty() {
                    parts.extend(calls.iter().map(|call|json!({"functionCall":{"id":call.id,"name":call.name,"args":call.arguments}})));
                } else {
                    parts.extend(continuation.gemini.iter().cloned());
                }
                contents.push(json!({"role":"model","parts":parts}));
            }
            Message::Tool { id, name, data, .. } => {
                let mut part =
                    json!({"functionResponse":{"name":name,"response":{"untrustedData":data}}});
                if native_ids.contains(id.as_str()) {
                    part["functionResponse"]["id"] = json!(id);
                }
                contents.push(json!({"role":"user","parts":[part]}));
            }
        }
    }
    let mut body = json!({"systemInstruction":{"parts":[{"text":INSTRUCTIONS}]},"contents":contents,"generationConfig":{"maxOutputTokens":request.max_output}});
    if !request.tools.is_empty() {
        body["tools"] = json!([{"functionDeclarations":request.tools.iter().map(|tool|json!({"name":tool.name,"description":tool.description,"parameters":tool.schema})).collect::<Vec<_>>()}]);
    }
    Ok((
        format!("models/{model}:streamGenerateContent?alt=sse"),
        body,
    ))
}

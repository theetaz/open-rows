// SPDX-License-Identifier: Apache-2.0
//! Carry signed model state only in native assistant-data fields, never instructions.
use crate::{
    ProviderKind,
    agent::{Continuation, ToolCall},
};
use serde_json::{Value, json};
pub(crate) enum Target {
    Responses,
    Gemini,
    Chat,
}
pub(crate) fn parts(kind: ProviderKind, value: &Value) -> Vec<(Target, Value)> {
    match kind {
        ProviderKind::OpenAi
            if value["type"] == "response.output_item.done"
                && value["item"]["type"] == "reasoning" =>
        {
            vec![(Target::Responses, value["item"].clone())]
        }
        ProviderKind::Gemini => value
            .pointer("/candidates/0/content/parts")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|part| part.get("functionCall").is_some())
            .map(|part| (Target::Gemini, part.clone()))
            .collect(),
        ProviderKind::OpenRouter | ProviderKind::Compatible | ProviderKind::LocalCompatible => {
            value
                .pointer("/choices/0/delta/reasoning_details")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|part| (Target::Chat, part.clone()))
                .collect()
        }
        ProviderKind::OpenAi | ProviderKind::Anthropic | ProviderKind::Ollama => vec![],
    }
}
pub(crate) fn chat_call(call: &ToolCall, kind: ProviderKind, continuation: &Continuation) -> Value {
    let mut value = json!({"id":call.id,"type":"function","function":{"name":call.name,"arguments":if kind==ProviderKind::Ollama {call.arguments.clone()} else {Value::String(call.arguments.to_string())}}});
    if let Some(extra) = continuation.chat_calls.get(&call.id) {
        value["extra_content"] = extra.clone();
    }
    value
}

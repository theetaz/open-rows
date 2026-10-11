// SPDX-License-Identifier: Apache-2.0
//! Accumulate protocol tool JSON under fixed caps; text is never parsed as a call.
use crate::{
    AiError, ProviderKind,
    agent::{Continuation, ToolCall, Turn},
};
use serde_json::Value;
use std::collections::BTreeMap;
#[derive(Default)]
struct PartialCall {
    id: String,
    name: String,
    arguments: String,
}
#[derive(Default)]
pub(crate) struct Calls {
    entries: BTreeMap<u64, PartialCall>,
    continuation: Continuation,
    metadata_bytes: usize,
}
impl Calls {
    pub(crate) fn frame(&mut self, kind: ProviderKind, value: &Value) -> Result<(), AiError> {
        self.metadata(kind, value)?;
        match kind {
            ProviderKind::OpenAi => self.openai(value)?,
            ProviderKind::Anthropic => self.anthropic(value)?,
            ProviderKind::OpenRouter | ProviderKind::Compatible | ProviderKind::LocalCompatible => {
                self.chat(value)?;
            }
            ProviderKind::Ollama => self.complete(value.pointer("/message/tool_calls"))?,
            ProviderKind::Gemini => self.gemini(value)?,
        }
        if self.entries.len() > 8
            || self.entries.values().any(|call| {
                call.id.len() > 128 || call.name.len() > 64 || call.arguments.len() > 32768
            })
        {
            return Err(AiError::InvalidResponse);
        }
        Ok(())
    }
    fn chat(&mut self, value: &Value) -> Result<(), AiError> {
        for call in value
            .pointer("/choices/0/delta/tool_calls")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let index = call["index"].as_u64().ok_or(AiError::InvalidResponse)?;
            let current = self.entries.entry(index).or_default();
            append(&mut current.id, call.get("id"))?;
            append(&mut current.name, call.pointer("/function/name"))?;
            append(&mut current.arguments, call.pointer("/function/arguments"))?;
            let id = current.id.clone();
            if let Some(extra) = call.get("extra_content") {
                self.charge_metadata(extra)?;
                self.continuation.chat_calls.insert(id, extra.clone());
            }
        }
        Ok(())
    }
    fn metadata(&mut self, kind: ProviderKind, value: &Value) -> Result<(), AiError> {
        let parts = super::continuation::parts(kind, value);
        for (target, part) in parts {
            self.charge_metadata(&part)?;
            match target {
                super::continuation::Target::Responses => self.continuation.responses.push(part),
                super::continuation::Target::Gemini => self.continuation.gemini.push(part),
                super::continuation::Target::Chat => self.continuation.chat.push(part),
            }
        }
        Ok(())
    }
    fn charge_metadata(&mut self, value: &Value) -> Result<(), AiError> {
        self.metadata_bytes += serde_json::to_vec(value)
            .map_err(|_| AiError::InvalidResponse)?
            .len();
        if self.metadata_bytes > 32768 {
            return Err(AiError::InvalidResponse);
        }
        Ok(())
    }
    fn gemini(&mut self, value: &Value) -> Result<(), AiError> {
        for call in value
            .pointer("/candidates/0/content/parts")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|part| part.get("functionCall"))
        {
            self.insert(call, "args")?;
        }
        Ok(())
    }
    fn openai(&mut self, value: &Value) -> Result<(), AiError> {
        match value["type"].as_str() {
            Some("response.output_item.added") if value["item"]["type"] == "function_call" => {
                let index = value["output_index"]
                    .as_u64()
                    .ok_or(AiError::InvalidResponse)?;
                self.entries.insert(
                    index,
                    PartialCall {
                        id: string(&value["item"]["call_id"])?,
                        name: string(&value["item"]["name"])?,
                        arguments: String::new(),
                    },
                );
            }
            Some("response.function_call_arguments.delta") => {
                let index = value["output_index"]
                    .as_u64()
                    .ok_or(AiError::InvalidResponse)?;
                let call = self
                    .entries
                    .get_mut(&index)
                    .ok_or(AiError::InvalidResponse)?;
                append(&mut call.arguments, value.get("delta"))?;
            }
            _ => {}
        }
        Ok(())
    }
    fn anthropic(&mut self, value: &Value) -> Result<(), AiError> {
        match value["type"].as_str() {
            Some("content_block_start") if value["content_block"]["type"] == "tool_use" => {
                let index = value["index"].as_u64().ok_or(AiError::InvalidResponse)?;
                self.entries.insert(
                    index,
                    PartialCall {
                        id: string(&value["content_block"]["id"])?,
                        name: string(&value["content_block"]["name"])?,
                        arguments: String::new(),
                    },
                );
            }
            Some("content_block_delta") if value["delta"]["type"] == "input_json_delta" => {
                let index = value["index"].as_u64().ok_or(AiError::InvalidResponse)?;
                let call = self
                    .entries
                    .get_mut(&index)
                    .ok_or(AiError::InvalidResponse)?;
                append(&mut call.arguments, value.pointer("/delta/partial_json"))?;
            }
            _ => {}
        }
        Ok(())
    }
    fn complete(&mut self, calls: Option<&Value>) -> Result<(), AiError> {
        for call in calls.and_then(Value::as_array).into_iter().flatten() {
            self.insert(&call["function"], "arguments")?;
        }
        Ok(())
    }
    fn insert(&mut self, call: &Value, field: &str) -> Result<(), AiError> {
        let index = u64::try_from(self.entries.len()).map_err(|_| AiError::InvalidResponse)?;
        self.entries.insert(
            index,
            PartialCall {
                id: call
                    .get("id")
                    .and_then(Value::as_str)
                    .map_or_else(|| format!("call-{index}"), str::to_owned),
                name: string(&call["name"])?,
                arguments: call[field].to_string(),
            },
        );
        Ok(())
    }
    pub(crate) fn finish(self) -> Result<Turn, AiError> {
        let calls = self
            .entries
            .into_values()
            .map(|call| {
                let arguments = serde_json::from_str::<Value>(&call.arguments)
                    .map_err(|_| AiError::InvalidResponse)?;
                if call.id.is_empty() || call.name.is_empty() || !arguments.is_object() {
                    return Err(AiError::InvalidResponse);
                }
                Ok(ToolCall {
                    id: call.id,
                    name: call.name,
                    arguments,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Turn {
            calls,
            continuation: self.continuation,
        })
    }
}
fn string(value: &Value) -> Result<String, AiError> {
    value
        .as_str()
        .map(str::to_owned)
        .ok_or(AiError::InvalidResponse)
}
fn append(output: &mut String, value: Option<&Value>) -> Result<(), AiError> {
    if let Some(value) = value {
        output.push_str(value.as_str().ok_or(AiError::InvalidResponse)?);
    }
    Ok(())
}

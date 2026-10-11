// SPDX-License-Identifier: Apache-2.0
//! Native request/stream formats; `OpenAI` uses Responses, compatible uses Chat.
use crate::{AiError, ProviderKind, TextRequest};
use serde_json::{Value, json};

pub(crate) fn request(
    kind: ProviderKind,
    request: &TextRequest,
) -> Result<(String, Value), AiError> {
    if request.model.is_empty()
        || request.model.len() > 256
        || request.context.text().len() > 1024 * 1024
        || !(1..=65536).contains(&request.max_output)
    {
        return Err(AiError::InvalidConfig);
    }
    let chat = json!({"model": request.model, "messages": [{"role": "user", "content": request.context.text()}], "stream": true, "max_tokens": request.max_output});
    let result = match kind {
        ProviderKind::Anthropic => ("messages".into(), chat),
        ProviderKind::OpenAi => (
            "responses".into(),
            json!({"model": request.model, "input": request.context.text(), "stream": true, "store": false, "max_output_tokens": request.max_output}),
        ),
        ProviderKind::OpenRouter | ProviderKind::Compatible | ProviderKind::LocalCompatible => {
            ("chat/completions".into(), chat)
        }
        ProviderKind::Ollama => (
            "api/chat".into(),
            json!({"model": request.model, "messages": [{"role": "user", "content": request.context.text()}], "stream": true, "options": {"num_predict": request.max_output}}),
        ),
        ProviderKind::Gemini => {
            let model = request
                .model
                .strip_prefix("models/")
                .unwrap_or(&request.model);
            if !model
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.'))
            {
                return Err(AiError::InvalidConfig);
            }
            (
                format!("models/{model}:streamGenerateContent?alt=sse"),
                json!({"contents": [{"role": "user", "parts": [{"text": request.context.text()}]}], "generationConfig": {"maxOutputTokens": request.max_output}}),
            )
        }
    };
    Ok(result)
}
pub(crate) fn delta(kind: ProviderKind, value: &Value) -> Result<(Vec<String>, bool), AiError> {
    if value.get("error").is_some()
        || matches!(
            value["type"].as_str(),
            Some("error" | "response.failed" | "response.incomplete")
        )
    {
        return Err(AiError::InvalidResponse);
    }
    let (text, done) = match kind {
        ProviderKind::OpenAi => (
            value
                .get("delta")
                .filter(|_| value["type"] == "response.output_text.delta"),
            value["type"] == "response.completed",
        ),
        ProviderKind::Anthropic => (
            value.pointer("/delta/text"),
            value["type"] == "message_stop",
        ),
        ProviderKind::OpenRouter | ProviderKind::Compatible | ProviderKind::LocalCompatible => {
            (value.pointer("/choices/0/delta/content"), false)
        }
        ProviderKind::Ollama => (value.pointer("/message/content"), value["done"] == true),
        ProviderKind::Gemini => return gemini_delta(value),
    };
    if let Some(text) = text {
        return Ok((
            vec![text.as_str().ok_or(AiError::InvalidResponse)?.into()],
            done,
        ));
    }
    let valid = value.get("type").is_some()
        || value.get("choices").is_some()
        || value.get("done").is_some();
    if !valid {
        return Err(AiError::InvalidResponse);
    }
    Ok((Vec::new(), done))
}
fn gemini_delta(value: &Value) -> Result<(Vec<String>, bool), AiError> {
    let candidates = value["candidates"]
        .as_array()
        .ok_or(AiError::InvalidResponse)?;
    let mut texts = Vec::new();
    let mut done = false;
    for candidate in candidates {
        let parts = candidate
            .pointer("/content/parts")
            .and_then(Value::as_array);
        for text in parts
            .into_iter()
            .flatten()
            .filter_map(|part| part.get("text"))
        {
            texts.push(text.as_str().ok_or(AiError::InvalidResponse)?.into());
        }
        done |= candidate.get("finishReason").is_some();
    }
    Ok((texts, done))
}

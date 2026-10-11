// SPDX-License-Identifier: Apache-2.0
//! Bounded discovery with pagination and honest fallback/unknown capabilities.
use crate::{
    AiError, Capabilities, Health, HttpProvider, Model, ProviderKind, Usage, Verification, http,
};
use serde_json::Value;

pub(crate) async fn verify(provider: &HttpProvider) -> Result<Verification, AiError> {
    let usage = if provider.kind == ProviderKind::OpenRouter {
        let value = get(provider, "key").await?;
        Some(usage(&value)?)
    } else {
        None
    };
    let mut path = if provider.kind == ProviderKind::Ollama {
        "api/tags"
    } else {
        "models"
    }
    .to_owned();
    let mut models = Vec::new();
    for _ in 0..20 {
        let value = match get(provider, &path).await {
            Err(AiError::Unsupported) if provider.kind == ProviderKind::Anthropic => {
                return Ok(fallback());
            }
            other => other?,
        };
        models.extend(parse(provider.kind, &value)?);
        if models.len() > 2000 {
            return Err(AiError::InvalidResponse);
        }
        let Some(next) = next_page(provider.kind, &value)? else {
            let health = if provider.kind == ProviderKind::Compatible && provider.http.key.is_none()
            {
                Health::Unknown
            } else {
                Health::Verified
            };
            return Ok(Verification {
                health,
                models,
                usage,
            });
        };
        path = next;
    }
    Err(AiError::InvalidResponse)
}
async fn get(provider: &HttpProvider, path: &str) -> Result<Value, AiError> {
    let mut exchange = provider.http.request(path, None).await?;
    http::read_json(&provider.http, &mut exchange).await
}
fn next_page(kind: ProviderKind, value: &Value) -> Result<Option<String>, AiError> {
    let token = match kind {
        ProviderKind::Anthropic if value["has_more"] == true => Some((
            "after_id",
            value["last_id"].as_str().ok_or(AiError::InvalidResponse)?,
        )),
        ProviderKind::Gemini => value
            .get("nextPageToken")
            .map(|v| {
                v.as_str()
                    .map(|token| ("pageToken", token))
                    .ok_or(AiError::InvalidResponse)
            })
            .transpose()?,
        ProviderKind::Anthropic
        | ProviderKind::OpenAi
        | ProviderKind::OpenRouter
        | ProviderKind::Compatible
        | ProviderKind::Ollama
        | ProviderKind::LocalCompatible => None,
    };
    Ok(token.map(|(name, token)| {
        format!(
            "models?{}",
            url::form_urlencoded::Serializer::new(String::new())
                .append_pair(name, token)
                .finish()
        )
    }))
}
fn parse(kind: ProviderKind, value: &Value) -> Result<Vec<Model>, AiError> {
    let field = if matches!(kind, ProviderKind::Gemini | ProviderKind::Ollama) {
        "models"
    } else {
        "data"
    };
    value[field]
        .as_array()
        .ok_or(AiError::InvalidResponse)?
        .iter()
        .map(|item| model(kind, item))
        .collect()
}
fn model(kind: ProviderKind, value: &Value) -> Result<Model, AiError> {
    let id_field = if matches!(kind, ProviderKind::Gemini | ProviderKind::Ollama) {
        "name"
    } else {
        "id"
    };
    let id = text(&value[id_field], 256)?;
    let name = value
        .get("display_name")
        .or_else(|| value.get("displayName"))
        .or_else(|| value.get("name"));
    let name = name
        .map(|value| text(value, 256))
        .transpose()?
        .unwrap_or_else(|| id.clone());
    let capabilities = capabilities(kind, value)?;
    Ok(Model {
        id,
        name,
        capabilities,
    })
}
fn text(value: &Value, max: usize) -> Result<String, AiError> {
    let text = value.as_str().ok_or(AiError::InvalidResponse)?;
    if text.is_empty() || text.len() > max || text.chars().any(char::is_control) {
        return Err(AiError::InvalidResponse);
    }
    Ok(text.into())
}
fn number(value: Option<&Value>) -> Result<Option<u32>, AiError> {
    value
        .filter(|v| !v.is_null())
        .map(|v| {
            v.as_u64()
                .and_then(|v| u32::try_from(v).ok())
                .ok_or(AiError::InvalidResponse)
        })
        .transpose()
}
fn capabilities(kind: ProviderKind, value: &Value) -> Result<Capabilities, AiError> {
    let mut capabilities = Capabilities {
        streaming: Some(true),
        ..Capabilities::default()
    };
    match kind {
        ProviderKind::Gemini => {
            capabilities.context_window = number(value.get("inputTokenLimit"))?;
            capabilities.max_output = number(value.get("outputTokenLimit"))?;
            capabilities.streaming = value
                .get("supportedGenerationMethods")
                .and_then(Value::as_array)
                .map(|methods| methods.iter().any(|method| method == "generateContent"));
        }
        ProviderKind::OpenRouter => {
            capabilities.context_window = number(value.get("context_length"))?;
            capabilities.max_output = number(value.pointer("/top_provider/max_completion_tokens"))?;
            let parameters = value.get("supported_parameters").and_then(Value::as_array);
            capabilities.tool_calling =
                parameters.map(|items| items.iter().any(|item| item == "tools"));
            capabilities.structured_output = parameters.map(|items| {
                items
                    .iter()
                    .any(|item| item == "response_format" || item == "structured_outputs")
            });
            capabilities.vision = value
                .pointer("/architecture/input_modalities")
                .and_then(Value::as_array)
                .map(|items| items.iter().any(|item| item == "image"));
        }
        ProviderKind::Anthropic => {
            capabilities.context_window = number(value.get("max_input_tokens"))?;
            capabilities.max_output = number(value.get("max_tokens"))?;
            capabilities.tool_calling = value
                .pointer("/capabilities/tool_calling/supported")
                .and_then(Value::as_bool);
            capabilities.structured_output = value
                .pointer("/capabilities/structured_outputs/supported")
                .and_then(Value::as_bool);
            capabilities.vision = value
                .pointer("/capabilities/image_input/supported")
                .and_then(Value::as_bool);
        }
        ProviderKind::OpenAi
        | ProviderKind::Compatible
        | ProviderKind::Ollama
        | ProviderKind::LocalCompatible => {}
    }
    Ok(capabilities)
}
fn usage(value: &Value) -> Result<Usage, AiError> {
    let data = value["data"].as_object().ok_or(AiError::InvalidResponse)?;
    let field = |name: &str| -> Result<Option<f64>, AiError> {
        data.get(name)
            .filter(|v| !v.is_null())
            .map(|v| {
                v.as_f64()
                    .filter(|n| n.is_finite() && *n >= 0.0)
                    .ok_or(AiError::InvalidResponse)
            })
            .transpose()
    };
    Ok(Usage {
        used: field("usage")?,
        limit: field("limit")?,
        remaining: field("limit_remaining")?,
    })
}
fn fallback() -> Verification {
    let models = ["claude-opus-5-5", "claude-sonnet-5-5", "claude-haiku-5-5"]
        .into_iter()
        .map(|id| Model {
            id: id.into(),
            name: id.into(),
            capabilities: Capabilities::default(),
        })
        .collect();
    Verification {
        health: Health::Unknown,
        models,
        usage: None,
    }
}

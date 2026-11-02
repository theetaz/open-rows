// SPDX-License-Identifier: Apache-2.0
//! Resolve saved model capabilities and credentials without exposing keys over IPC.
use super::{AiState, config, secret_work};
use crate::connections::IpcError;
use openrows_ai::{
    HttpProvider, NetworkOptions, ProviderKind, agent::AgentProvider, codex::CodexProvider,
};
use openrows_secrets::SecretKey;
use std::sync::{Arc, PoisonError};
use tauri::{AppHandle, Manager};
/// Build the saved provider; `tools` false forces a single-shot round.
pub(crate) async fn provider(
    app: &AppHandle,
    id: &str,
    model: &str,
    tools: bool,
) -> Result<Box<dyn AgentProvider>, IpcError> {
    let config = config(app, id)?;
    let state = app.state::<AiState>();
    if config.kind == ProviderKind::Codex {
        let bridge = super::codex::bridge(app, &config)?;
        return Ok(Box::new(
            CodexProvider::new(bridge, config, state.control.clone()).with_tool_calling(tools),
        ));
    }
    let supported = state
        .models
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(id)
        .and_then(|result| result.models.iter().find(|item| item.id == model))
        .is_some_and(|item| item.capabilities.tool_calling == Some(true));
    let secrets = Arc::clone(&state.secrets);
    let provider_id = id.to_owned();
    let key = if config.secret_ref.is_empty() {
        None
    } else {
        secret_work(move || secrets.get(&SecretKey::ai_provider(&provider_id))).await?
    };
    Ok(Box::new(
        HttpProvider::new(
            &config,
            key,
            state.control.clone(),
            NetworkOptions::default(),
        )?
        .with_tool_calling(supported && tools),
    ))
}

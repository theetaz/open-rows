// SPDX-License-Identifier: Apache-2.0
//! Resolve saved model capabilities and credentials without exposing keys over IPC.
use super::{AiState, config, secret_work};
use crate::connections::IpcError;
use openrows_ai::{HttpProvider, NetworkOptions};
use openrows_secrets::SecretKey;
use std::sync::{Arc, PoisonError};
use tauri::{AppHandle, Manager};
pub(crate) async fn provider(
    app: &AppHandle,
    id: &str,
    model: &str,
) -> Result<HttpProvider, IpcError> {
    let config = config(app, id)?;
    let state = app.state::<AiState>();
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
    Ok(HttpProvider::new(
        &config,
        key,
        state.control.clone(),
        NetworkOptions::default(),
    )?
    .with_tool_calling(supported))
}

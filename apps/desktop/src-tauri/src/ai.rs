// SPDX-License-Identifier: Apache-2.0
//! Saved AI metadata, capability cache and keychain-backed native provider construction.
#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri owns command arguments"
)]
pub(crate) mod codex;
mod generation;
use crate::security::secret_prompt::{NativePrompt, PromptText, SecretEntry, SecretPrompt, enter};
use crate::{
    connections::{IpcError, IpcErrorKind},
    settings::{self, SettingsFile},
};
pub(crate) use generation::provider as generation_provider;
use openrows_ai::{
    AiControl, AiError, AiSettings, Cancellation, Health, HttpProvider, NetworkOptions, Provider,
    ProviderConfig, RequestOptions, Verification,
};
use openrows_secrets::{CachingSecretStore, KeychainSecretStore, SecretKey, SecretStore};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, PoisonError},
};
use tauri::{AppHandle, Manager};

/// App-owned permission and cancellable metadata probes, with existing keychain backend.
pub(crate) struct AiState {
    pub(crate) control: AiControl,
    secrets: Arc<dyn SecretStore>,
    known: Mutex<AiSettings>,
    runs: Mutex<HashMap<String, Cancellation>>,
    models: Mutex<HashMap<String, Verification>>,
}
impl Default for AiState {
    fn default() -> Self {
        Self {
            control: AiControl::default(),
            secrets: Arc::new(CachingSecretStore::new(KeychainSecretStore::new())),
            runs: Mutex::default(),
            models: Mutex::default(),
            known: Mutex::default(),
        }
    }
}
impl AiState {
    pub(crate) fn configure(&self, value: &serde_json::Value) {
        let settings = parse(value).unwrap_or_default();
        let mut known = self.known.lock().unwrap_or_else(PoisonError::into_inner);
        if *known != settings {
            self.control.set_enabled(false);
            known.clone_from(&settings);
            self.models
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clear();
        }
        self.control.set_enabled(settings.enabled);
    }
    pub(crate) fn revoke(&self) {
        self.control.set_enabled(false);
    }
    /// Cancel every verification and sign-in when the app quits.
    pub(crate) fn cancel_all(&self) {
        for cancel in self
            .runs
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .values()
        {
            cancel.cancel();
        }
    }
    fn cancel(&self, id: &str) {
        if let Some(cancel) = self
            .runs
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(id)
        {
            cancel.cancel();
        }
    }
}
pub(crate) fn parse(value: &serde_json::Value) -> Result<AiSettings, AiError> {
    let settings: AiSettings = value
        .get("ai")
        .map(|value| serde_json::from_value(value.clone()))
        .transpose()
        .map_err(|_| AiError::InvalidConfig)?
        .unwrap_or_default();
    settings.validate()?;
    Ok(settings)
}
/// Validate settings before writing so credentials cannot be persisted via generic IPC.
pub(crate) fn validate_settings(value: &serde_json::Value) -> Result<(), IpcError> {
    parse(value).map(|_| ()).map_err(IpcError::from)
}
pub(crate) fn config(app: &AppHandle, id: &str) -> Result<ProviderConfig, IpcError> {
    openrows_ai::validate_id(id)?;
    let value = settings::current(&app.state::<SettingsFile>()).unwrap_or_default();
    parse(&value)?
        .providers
        .into_iter()
        .find(|config| config.id == id)
        .ok_or_else(|| IpcError::new(IpcErrorKind::NotFound, "AI provider not found"))
}
/// Whole-loop deadline for this provider: the user's CLI setting for Codex,
/// the fixed 60-second default for every HTTP provider.
pub(crate) fn turn_deadline(app: &AppHandle, id: &str) -> Result<std::time::Duration, IpcError> {
    let default = openrows_ai::agent::AgentLimits::default().timeout;
    if config(app, id)?.kind != openrows_ai::ProviderKind::Codex {
        return Ok(default);
    }
    let value = settings::current(&app.state::<SettingsFile>()).unwrap_or_default();
    Ok(parse(&value).map_or(default, |settings| settings.cli_turn()))
}
impl From<AiError> for IpcError {
    fn from(error: AiError) -> Self {
        let kind = match error {
            AiError::Auth | AiError::Quota => IpcErrorKind::Auth,
            AiError::RateLimit { .. } | AiError::Network => IpcErrorKind::Network,
            AiError::Unsupported | AiError::Disabled => IpcErrorKind::Unsupported,
            AiError::InvalidResponse => IpcErrorKind::Protocol,
            AiError::Audit => IpcErrorKind::Storage,
            AiError::InvalidConfig => IpcErrorKind::Invalid,
            AiError::Cancelled => IpcErrorKind::Cancelled,
            AiError::Timeout => IpcErrorKind::Timeout,
        };
        Self::new(kind, error.to_string())
    }
}
fn secret_error() -> IpcError {
    IpcError::new(IpcErrorKind::Secret, "AI keychain operation failed")
}
async fn secret_work<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, openrows_secrets::SecretError> + Send + 'static,
) -> Result<T, IpcError> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|_| secret_error())?
        .map_err(|_| secret_error())
}

/// Enter a replacement key in the native secure dialog. The key goes from the
/// dialog straight to the keychain; only the outcome crosses IPC, and there is
/// no command to reveal or return it.
#[tauri::command]
#[specta::specta]
pub(crate) async fn ai_key_prompt(app: AppHandle, id: String) -> Result<SecretEntry, IpcError> {
    app.state::<crate::security::Security>()
        .admit("ai_key_prompt")?;
    let config = config(&app, &id)?;
    let prompt = NativePrompt { app: app.clone() };
    let secrets = Arc::clone(&app.state::<AiState>().secrets);
    let entry = prompt_key(&prompt, secrets, &config).await?;
    if entry == SecretEntry::Saved {
        // Work started with the previous key stops; new work reads the new one.
        app.state::<AiState>().cancel(&id);
        app.state::<crate::assistant::AssistantState>().cancel(None);
        app.state::<crate::assistant::ScheduledState>()
            .interrupt_all();
    }
    Ok(entry)
}
/// Ask for and store one provider key; keyless and CLI providers have none.
async fn prompt_key(
    prompt: &dyn SecretPrompt,
    secrets: Arc<dyn SecretStore>,
    config: &ProviderConfig,
) -> Result<SecretEntry, IpcError> {
    if config.secret_ref.is_empty() || config.kind == openrows_ai::ProviderKind::Codex {
        return Err(AiError::InvalidConfig.into());
    }
    let text = PromptText {
        title: format!("API key for {}", config.name),
        message: "OpenRows stores the key only in the macOS keychain. It is never shown again \
                  or sent to the app window."
            .into(),
    };
    let id = config.id.clone();
    enter(prompt, &text, move |key| {
        Box::pin(secret_work(move || {
            secrets.set(&SecretKey::ai_provider(&id), &key)
        }))
    })
    .await
}
/// Remove the key before removing its reference from settings.
#[tauri::command]
#[specta::specta]
pub(crate) async fn ai_key_delete(app: AppHandle, id: String) -> Result<(), IpcError> {
    app.state::<crate::security::Security>()
        .admit("ai_key_delete")?;
    let config = config(&app, &id)?;
    let state = app.state::<AiState>();
    state.cancel(&id);
    if config.kind == openrows_ai::ProviderKind::Codex {
        return codex::forget(&app, &config);
    }
    app.state::<crate::assistant::AssistantState>().cancel(None);
    app.state::<crate::assistant::ScheduledState>()
        .interrupt_all();
    let secrets = Arc::clone(&state.secrets);
    secret_work(move || secrets.delete(&SecretKey::ai_provider(&id))).await
}
/// Cancel this provider's current verification request without changing settings.
#[tauri::command]
#[specta::specta]
pub(crate) fn ai_cancel(app: AppHandle, id: String) -> Result<(), IpcError> {
    app.state::<crate::security::Security>()
        .admit("ai_cancel")?;
    openrows_ai::validate_id(&id)?;
    app.state::<AiState>().cancel(&id);
    Ok(())
}
struct Run<'a> {
    state: &'a AiState,
    id: String,
}
impl Drop for Run<'_> {
    fn drop(&mut self) {
        self.state
            .runs
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&self.id);
    }
}
fn start<'a>(state: &'a AiState, id: &str) -> Result<(Run<'a>, Cancellation), IpcError> {
    let mut runs = state.runs.lock().unwrap_or_else(PoisonError::into_inner);
    if runs.contains_key(id) {
        return Err(IpcError::invalid("AI verification already running"));
    }
    let cancel = Cancellation::default();
    runs.insert(id.into(), cancel.clone());
    Ok((
        Run {
            state,
            id: id.into(),
        },
        cancel,
    ))
}
/// Verify only saved metadata; permission is checked natively and in the provider.
#[tauri::command]
#[specta::specta]
pub(crate) async fn ai_verify(app: AppHandle, id: String) -> Result<Verification, IpcError> {
    app.state::<crate::security::Security>()
        .admit("ai_verify")?;
    let config = config(&app, &id)?;
    let state = app.state::<AiState>();
    let value = settings::current(&app.state::<SettingsFile>()).unwrap_or_default();
    state.configure(&value);
    if !state.control.enabled() {
        return Err(AiError::Disabled.into());
    }
    let (_run, cancel) = start(&state, &id)?;
    let result = if config.kind == openrows_ai::ProviderKind::Codex {
        Ok(codex::verify(&app, &config, cancel).await?)
    } else {
        verify_http(&state, &config, cancel).await?
    };
    if let Ok(result) = &result {
        state
            .models
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(config.id.clone(), result.clone());
    }
    app.state::<crate::security::Security>()
        .admit("ai_verify")?;
    Ok(result.unwrap_or_else(|reason| Verification {
        health: Health::Failed { reason },
        models: Vec::new(),
        usage: None,
        cli: None,
    }))
}
async fn verify_http(
    state: &AiState,
    config: &ProviderConfig,
    cancel: Cancellation,
) -> Result<Result<Verification, AiError>, IpcError> {
    let secrets = Arc::clone(&state.secrets);
    let id = config.id.clone();
    let key = if config.secret_ref.is_empty() {
        None
    } else {
        secret_work(move || secrets.get(&SecretKey::ai_provider(&id))).await?
    };
    let provider = HttpProvider::new(
        config,
        key,
        state.control.clone(),
        NetworkOptions::default(),
    )?;
    let options = RequestOptions {
        cancel,
        ..RequestOptions::default()
    };
    Ok(provider.verify(&options).await)
}
#[cfg(test)]
mod tests;

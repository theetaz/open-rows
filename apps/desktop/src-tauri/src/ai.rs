// SPDX-License-Identifier: Apache-2.0
//! AI metadata verification; no generation/context command is exposed to IPC.
#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri owns command arguments"
)]
use crate::{
    connections::{IpcError, IpcErrorKind},
    settings::{self, SettingsFile},
};
use openrows_ai::{
    AiControl, AiError, AiSettings, Cancellation, Health, HttpProvider, NetworkOptions, Provider,
    ProviderConfig, RequestOptions, Verification,
};
use openrows_secrets::{
    CachingSecretStore, KeychainSecretStore, SecretKey, SecretStore, SecretString,
};
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
}
impl Default for AiState {
    fn default() -> Self {
        Self {
            control: AiControl::default(),
            secrets: Arc::new(CachingSecretStore::new(KeychainSecretStore::new())),
            runs: Mutex::default(),
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
        }
        self.control.set_enabled(settings.enabled);
    }
    pub(crate) fn revoke(&self) {
        self.control.set_enabled(false);
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

/// Store a replacement key; there is no command to reveal or return it.
#[tauri::command]
#[specta::specta]
pub(crate) async fn ai_key_set(app: AppHandle, id: String, key: String) -> Result<(), IpcError> {
    app.state::<crate::security::Security>()
        .admit("ai_key_set")?;
    let config = config(&app, &id)?;
    let key = SecretString::new(key);
    if key.expose().is_empty()
        || key.expose().len() > 4096
        || key.expose().chars().any(char::is_control)
    {
        return Err(AiError::InvalidConfig.into());
    }
    if config.secret_ref.is_empty() {
        return Err(AiError::InvalidConfig.into());
    }
    let state = app.state::<AiState>();
    state.cancel(&id);
    let secrets = Arc::clone(&state.secrets);
    secret_work(move || secrets.set(&SecretKey::ai_provider(&id), &key)).await
}
/// Remove the key before removing its reference from settings.
#[tauri::command]
#[specta::specta]
pub(crate) async fn ai_key_delete(app: AppHandle, id: String) -> Result<(), IpcError> {
    app.state::<crate::security::Security>()
        .admit("ai_key_delete")?;
    config(&app, &id)?;
    let state = app.state::<AiState>();
    state.cancel(&id);
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
    let secrets = Arc::clone(&state.secrets);
    let key = if config.secret_ref.is_empty() {
        None
    } else {
        secret_work(move || secrets.get(&SecretKey::ai_provider(&id))).await?
    };
    let provider = HttpProvider::new(
        &config,
        key,
        state.control.clone(),
        NetworkOptions::default(),
    )?;
    let options = RequestOptions {
        cancel,
        ..RequestOptions::default()
    };
    let result = provider.verify(&options).await;
    app.state::<crate::security::Security>()
        .admit("ai_verify")?;
    Ok(result.unwrap_or_else(|reason| Verification {
        health: Health::Failed { reason },
        models: Vec::new(),
        usage: None,
    }))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn secrets_are_rejected_by_settings_and_ipc_errors_are_fixed() {
        assert!(validate_settings(&serde_json::json!({"ai":{"apiKey":"test-only"}})).is_err());
        assert!(
            validate_settings(&serde_json::json!({"ai":{"providers":[{"key":"test-only"}]}}))
                .is_err()
        );
        let error = IpcError::from(AiError::Auth);
        assert_eq!(error.kind, IpcErrorKind::Auth);
        assert!(!serde_json::to_string(&error).unwrap().contains("test-only"));
    }
    #[test]
    fn global_permission_defaults_off_and_invalid_settings_revoke_it() {
        let state = AiState::default();
        assert!(!state.control.enabled());
        state.configure(&serde_json::json!({"ai":{"enabled":true}}));
        assert!(state.control.enabled());
        state.configure(&serde_json::json!({"ai":{"enabled":true,"key":"test-only"}}));
        assert!(!state.control.enabled());
    }
}

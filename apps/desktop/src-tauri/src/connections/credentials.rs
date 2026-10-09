// SPDX-License-Identifier: Apache-2.0
//! Native blocking trust confirmation and cancellable credential attempts.
#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri deserialises command arguments into owned values"
)]
use super::{
    Connections,
    dto::ProfileInput,
    error::{IpcError, IpcErrorKind},
    profiles::profile_id,
};
use openrows_secrets::SecretString;
use openrows_store::profile::{
    CommandCancellation, CommandTrust, ConnectionProfile, CredentialCommand, CredentialError,
    PasswordStorage, ProfileId, run_credential,
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tauri::State;
use tauri_plugin_dialog::{DialogExt as _, MessageDialogButtons};

/// Live command attempts, cancelled through the existing connection progress action.
#[derive(Default)]
pub(crate) struct CredentialRuns(Mutex<HashMap<ProfileId, CommandCancellation>>);
impl CredentialRuns {
    pub(crate) fn cancel(&self, id: ProfileId) {
        if let Ok(runs) = self.0.lock()
            && let Some(cancel) = runs.get(&id)
        {
            cancel.cancel();
        }
    }
    fn start(&self, id: ProfileId) -> Result<CommandCancellation, IpcError> {
        let mut runs = self.0.lock().map_err(|_| {
            IpcError::new(IpcErrorKind::Internal, "Credential attempts unavailable")
        })?;
        if runs.contains_key(&id) {
            return Err(IpcError::invalid("Credential command is already running"));
        }
        let cancel = CommandCancellation::default();
        runs.insert(id, cancel.clone());
        Ok(cancel)
    }
    fn finish(&self, id: ProfileId) {
        if let Ok(mut runs) = self.0.lock() {
            runs.remove(&id);
        }
    }
}
impl From<CredentialError> for IpcError {
    fn from(error: CredentialError) -> Self {
        let kind = match error {
            CredentialError::Timeout => IpcErrorKind::Timeout,
            CredentialError::Cancelled => IpcErrorKind::Cancelled,
            CredentialError::Profile(error) => return error.into(),
            CredentialError::Untrusted
            | CredentialError::Process(_)
            | CredentialError::Failed(_) => IpcErrorKind::Auth,
        };
        Self::new(kind, error.to_string())
    }
}
/// Explicit confirmation always shows the exact program and JSON argument vector.
pub(crate) async fn confirm(
    app: &tauri::AppHandle,
    command: &CredentialCommand,
) -> Result<(), IpcError> {
    command.validate()?;
    let args =
        serde_json::to_string(&command.args).map_err(|_| IpcError::invalid("Invalid arguments"))?;
    let program = serde_json::to_string(&command.program)
        .map_err(|_| IpcError::invalid("Invalid program path"))?;
    let text = format!(
        "This program can access files and the network with your permissions. Only trust commands you recognize. Arguments are literal and must not contain secrets.\n\nProgram: {program}\nArguments: {args}"
    );
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.dialog()
        .message(text)
        .title("Trust credential command?")
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Trust and run".into(),
            "Cancel".into(),
        ))
        .show(move |answer| {
            let _ = sender.send(answer);
        });
    if receiver.await.unwrap_or(false) {
        Ok(())
    } else {
        Err(IpcError::new(
            IpcErrorKind::Cancelled,
            "Credential command was not trusted",
        ))
    }
}
/// Trust a newly created or edited command only after the native confirmation.
pub(crate) async fn confirm_edit(
    app: &tauri::AppHandle,
    profile: &ConnectionProfile,
    before: Option<&ConnectionProfile>,
) -> Result<bool, IpcError> {
    if profile.password_storage != PasswordStorage::Command {
        return Ok(false);
    }
    let command = profile
        .credential_command
        .as_ref()
        .ok_or_else(|| IpcError::invalid("Choose a credential program"))?;
    if before.is_some_and(|before| {
        before.credential_command.as_ref() == Some(command)
            && before.password_storage == PasswordStorage::Command
    }) {
        return Ok(false);
    }
    confirm(app, command).await?;
    Ok(true)
}
/// Resolve a command password for this individual connection attempt; never cache it.
pub(crate) async fn password(
    state: &Connections,
    app: &tauri::AppHandle,
    profile: &ConnectionProfile,
) -> Result<SecretString, IpcError> {
    let command = profile
        .credential_command
        .as_ref()
        .ok_or_else(|| IpcError::invalid("Choose a credential program"))?
        .clone();
    let id = profile.id;
    let cancel = state.credential_runs.start(id)?;
    let result = execute(state, app, (id, command), cancel).await;
    state.credential_runs.finish(id);
    result
}
async fn execute(
    state: &Connections,
    app: &tauri::AppHandle,
    invocation: (ProfileId, CredentialCommand),
    cancel: CommandCancellation,
) -> Result<SecretString, IpcError> {
    let (id, command) = invocation;
    if !state.command_trust.is_trusted(id, &command)? {
        confirm(app, &command).await?;
        if cancel.is_cancelled() {
            return Err(CredentialError::Cancelled.into());
        }
        state.command_trust.trust(id, &command)?;
    }
    let trust: Arc<CommandTrust> = Arc::clone(&state.command_trust);
    tauri::async_runtime::spawn_blocking(move || run_credential(&trust, id, &command, &cancel))
        .await
        .map_err(|_| IpcError::new(IpcErrorKind::Internal, "Credential worker failed"))?
        .map_err(IpcError::from)
}
/// Test the draft command with trust, returning success without its value.
#[tauri::command]
#[specta::specta]
pub(crate) async fn credential_test(
    app: tauri::AppHandle,
    state: State<'_, Connections>,
    profile_id: Option<String>,
    input: ProfileInput,
) -> Result<(), IpcError> {
    let mut profile = input.new_profile();
    input.apply(&mut profile)?;
    if let Some(id) = profile_id {
        profile.id = self::profile_id(&id)?;
    }
    password(&state, &app, &profile).await.map(|_| ())
}
/// Cancel the profile's current credential command, including pending trust.
#[tauri::command]
#[specta::specta]
pub(crate) fn credential_cancel(
    state: State<'_, Connections>,
    profile_id: String,
) -> Result<(), IpcError> {
    state.credential_runs.cancel(self::profile_id(&profile_id)?);
    Ok(())
}

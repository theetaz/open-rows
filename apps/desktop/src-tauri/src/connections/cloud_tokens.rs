// SPDX-License-Identifier: Apache-2.0
//! Cloud IAM tokens on the connect path (FR-CONN-11, docs/24 §19): check the
//! route, get the user's approval of the command and destination once, then
//! ask the signed-in CLI for a new token on every connect and reconnect.
use std::io;
use std::sync::Arc;

use openrows_driver_api::DriverError;
use openrows_secrets::SecretString;
use openrows_store::profile::{
    CloudAuth, CloudAuthError, CloudInvocation, CommandCancellation, CommandPurpose,
    ConnectionProfile, ProfileId,
};
use tauri::Manager as _;

use super::error::{IpcError, IpcErrorKind};
use super::resilient::FreshPassword;
use super::{Connections, helpers};

impl From<CloudAuthError> for IpcError {
    fn from(error: CloudAuthError) -> Self {
        let kind = match &error {
            CloudAuthError::NotInstalled { .. } => IpcErrorKind::NotFound,
            CloudAuthError::InsecureRoute { .. } => IpcErrorKind::Tls,
            CloudAuthError::Timeout { .. } => IpcErrorKind::Timeout,
            CloudAuthError::Cancelled => IpcErrorKind::Cancelled,
            CloudAuthError::Profile(_) => IpcErrorKind::Invalid,
            CloudAuthError::SignedOut { .. }
            | CloudAuthError::WrongTarget { .. }
            | CloudAuthError::PermissionDenied { .. }
            | CloudAuthError::Untrusted
            | CloudAuthError::Failed { .. } => IpcErrorKind::Auth,
        };
        Self::new(kind, error.to_string())
    }
}

/// A token for one connect attempt, asking for approval when this command and
/// destination have none yet. Never cached.
pub(crate) async fn password(
    state: &Connections,
    profile: &ConnectionProfile,
) -> Result<SecretString, IpcError> {
    let invocation = invocation(state, profile)?;
    let cancel = state.credential_runs.join(profile.id)?;
    let result = async {
        approve(state, profile.id, &invocation, &cancel).await?;
        Ok(run(state, profile.id, invocation, cancel).await?)
    }
    .await;
    state.credential_runs.finish(profile.id);
    result
}

/// What a reconnect calls. It never opens a dialog in the background: a
/// destination edited since the session opened fails with a typed error.
pub(crate) fn fresh_password(state: &Connections, profile: ConnectionProfile) -> FreshPassword {
    let app = state.app.clone();
    Arc::new(move || {
        let (app, profile) = (app.clone(), profile.clone());
        Box::pin(async move {
            let state = app.state::<Connections>();
            let invocation = invocation(&state, &profile).map_err(login_failure)?;
            let cancel = state
                .credential_runs
                .join(profile.id)
                .map_err(login_failure)?;
            let result = run(&state, profile.id, invocation, cancel).await;
            state.credential_runs.finish(profile.id);
            result
                .map(|token| token.expose().into())
                .map_err(|error| reconnect_failure(&error))
        })
    })
}

fn invocation(
    state: &Connections,
    profile: &ConnectionProfile,
) -> Result<CloudInvocation, IpcError> {
    let tool = profile.cloud_auth.as_ref().map(CloudAuth::tool);
    let override_path = tool.and_then(|tool| state.helper_settings.path(tool));
    Ok(CloudInvocation::new(
        profile,
        override_path.as_deref(),
        &helpers::tool_dirs(),
    )?)
}

async fn approve(
    state: &Connections,
    id: ProfileId,
    invocation: &CloudInvocation,
    cancel: &CommandCancellation,
) -> Result<(), IpcError> {
    if invocation.is_trusted(&state.command_trust, id)? {
        return Ok(());
    }
    let title = super::credentials::title(CommandPurpose::CloudToken);
    super::credentials::ask(&state.app, title, approval_text(invocation)?).await?;
    if cancel.is_cancelled() {
        return Err(CloudAuthError::Cancelled.into());
    }
    invocation.trust(&state.command_trust, id)?;
    Ok(())
}

/// Azure and Google tokens also work on the user's other databases, so the
/// question names the host that receives one, not only the program.
fn approval_text(invocation: &CloudInvocation) -> Result<String, IpcError> {
    let command = invocation.command();
    let args =
        serde_json::to_string(&command.args).map_err(|_| IpcError::invalid("Invalid arguments"))?;
    let program = serde_json::to_string(&command.program)
        .map_err(|_| IpcError::invalid("Invalid program path"))?;
    Ok(format!(
        "OpenRows will run this program with your signed-in cloud account and send the token it returns to {}. The token can sign in as you until it expires, so only continue if you recognize this database host.\n\nProgram: {program}\nArguments: {args}",
        invocation.destination()
    ))
}

async fn run(
    state: &Connections,
    id: ProfileId,
    invocation: CloudInvocation,
    cancel: CommandCancellation,
) -> Result<SecretString, CloudAuthError> {
    let trust = Arc::clone(&state.command_trust);
    tauri::async_runtime::spawn_blocking(move || invocation.run(&trust, id, &cancel))
        .await
        .map_err(|_| CloudAuthError::Cancelled)?
}

fn login_failure(error: IpcError) -> DriverError {
    DriverError::Auth(error.message)
}

/// A slow or unexplained CLI failure may pass (the network after a wake), so
/// the session retries it; being signed out or refused needs the user.
fn reconnect_failure(error: &CloudAuthError) -> DriverError {
    let message = error.to_string();
    match error {
        CloudAuthError::Timeout { .. } | CloudAuthError::Failed { .. } => {
            DriverError::Network(io::Error::other(message))
        }
        _ => DriverError::Auth(message),
    }
}

#[cfg(all(test, unix))]
#[path = "cloud_tokens_tests.rs"]
mod tests;

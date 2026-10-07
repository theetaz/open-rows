// SPDX-License-Identifier: Apache-2.0
//! User authentication for one hop: methods are tried in the configured
//! order; partial success (e.g. key then OTP) continues with the next method.

mod agent;
mod bigint;
mod interactive;
mod key;
mod rsa;

use std::path::PathBuf;

use russh::{
    client::{AuthResult, Handle},
    keys::{HashAlg, PrivateKeyWithHashAlg},
};

pub use agent::AgentSource;
pub use interactive::{Challenge, ChallengePrompt, KeyboardInteractivePrompter, PromptCancelled};
use key::LoadedKey;

use crate::{SecretString, SshError, handler::ClientHandler, timeouts::PromptClock};

/// One way to prove the user's identity to a hop.
///
/// `Debug` never shows secrets ([`SecretString`] is redacted).
#[derive(Clone, Debug)]
pub enum AuthMethod {
    /// Password authentication.
    Password(SecretString),
    /// A private key file, with its passphrase if encrypted.
    PrivateKey {
        /// Key file path.
        path: PathBuf,
        /// Passphrase for encrypted OpenSSH keys.
        passphrase: Option<SecretString>,
    },
    /// Keys held by an ssh-agent.
    Agent(AgentSource),
    /// Keyboard-interactive challenges answered by the prompter.
    KeyboardInteractive,
}

impl AuthMethod {
    pub(crate) fn label(&self) -> &'static str {
        match self {
            Self::Password(_) => "password",
            Self::PrivateKey { .. } => "publickey",
            Self::Agent(_) => "agent",
            Self::KeyboardInteractive => "keyboard-interactive",
        }
    }
}

/// Result of one method.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AuthOutcome {
    Success,
    /// Accepted, but the server requires another method too.
    Partial,
    Rejected,
}

impl AuthOutcome {
    fn from_partial(partial_success: bool) -> Self {
        if partial_success {
            Self::Partial
        } else {
            Self::Rejected
        }
    }
}

impl From<AuthResult> for AuthOutcome {
    fn from(result: AuthResult) -> Self {
        match result {
            AuthResult::Success => Self::Success,
            AuthResult::Failure {
                partial_success, ..
            } => Self::from_partial(partial_success),
        }
    }
}

/// What authentication needs besides the session handle.
pub(crate) struct AuthContext<'a> {
    pub(crate) host: &'a str,
    pub(crate) user: &'a str,
    pub(crate) prompter: &'a dyn KeyboardInteractivePrompter,
    pub(crate) clock: &'a PromptClock,
}

/// Try `methods` in order until the server reports success.
///
/// Local problems with one method (missing agent, unreadable key) do not stop
/// the others; if nothing succeeds, the first such problem is returned so the
/// UI can act on it (e.g. ask for a passphrase), otherwise `AuthFailed`.
pub(crate) async fn authenticate(
    handle: &mut Handle<ClientHandler>,
    methods: &[AuthMethod],
    context: &AuthContext<'_>,
) -> Result<(), SshError> {
    let mut tried = Vec::new();
    let mut local_error = None;
    for method in methods {
        tried.push(method.label());
        match attempt(handle, method, context).await {
            Ok(AuthOutcome::Success) => return Ok(()),
            Ok(AuthOutcome::Partial | AuthOutcome::Rejected) => {}
            Err(error) if is_local(&error) => {
                local_error.get_or_insert(error);
            }
            Err(error) => return Err(error),
        }
    }
    Err(local_error.unwrap_or_else(|| SshError::AuthFailed {
        user: context.user.to_owned(),
        host: context.host.to_owned(),
        tried: tried.join(", "),
    }))
}

async fn attempt(
    handle: &mut Handle<ClientHandler>,
    method: &AuthMethod,
    context: &AuthContext<'_>,
) -> Result<AuthOutcome, SshError> {
    match method {
        AuthMethod::Password(password) => Ok(handle
            .authenticate_password(context.user, password.expose())
            .await?
            .into()),
        AuthMethod::PrivateKey { path, passphrase } => {
            match key::load(path, passphrase.as_ref()).await? {
                LoadedKey::Native(key) => {
                    let key = PrivateKeyWithHashAlg::new(key, None);
                    Ok(handle
                        .authenticate_publickey(context.user, key)
                        .await?
                        .into())
                }
                LoadedKey::Rsa(mut signer) => {
                    let hash = rsa_hash(handle).await?;
                    let result = handle
                        .authenticate_publickey_with(
                            context.user,
                            signer.public_key(),
                            hash,
                            &mut *signer,
                        )
                        .await
                        .map_err(|error| SshError::KeyLoad {
                            path: path.clone(),
                            reason: error.to_string(),
                        })?;
                    Ok(result.into())
                }
            }
        }
        AuthMethod::Agent(source) => agent::authenticate(handle, context.user, source).await,
        AuthMethod::KeyboardInteractive => interactive::authenticate(handle, context).await,
    }
}

/// Best RSA signature hash the server accepts (`server-sig-algs`, RFC 8308).
async fn rsa_hash(handle: &Handle<ClientHandler>) -> Result<Option<HashAlg>, SshError> {
    Ok(handle.best_supported_rsa_hash().await?.flatten())
}

fn is_local(error: &SshError) -> bool {
    matches!(
        error,
        SshError::PassphraseRequired { .. }
            | SshError::KeyLoad { .. }
            | SshError::AgentNotConfigured
            | SshError::Agent { .. }
    )
}

#[cfg(test)]
mod tests;

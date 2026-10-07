// SPDX-License-Identifier: Apache-2.0
//! ssh-agent authentication over `SSH_AUTH_SOCK` or an explicit socket
//! (macOS launchd agent, 1Password, Secretive all expose a Unix socket).

use std::path::PathBuf;

use russh::client::Handle;

use super::AuthOutcome;
use crate::{SshError, handler::ClientHandler};

/// Where to find the agent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AgentSource {
    /// The `SSH_AUTH_SOCK` environment variable.
    Environment,
    /// An explicit socket path (e.g. a 1Password or Secretive agent).
    Socket(PathBuf),
}

/// Offer each agent identity in turn until one is accepted.
#[cfg(unix)]
pub(crate) async fn authenticate(
    handle: &mut Handle<ClientHandler>,
    user: &str,
    source: &AgentSource,
) -> Result<AuthOutcome, SshError> {
    use russh::keys::agent::client::AgentClient;

    use super::rsa_hash;

    let mut agent = match source {
        AgentSource::Environment => AgentClient::connect_env().await,
        AgentSource::Socket(path) => AgentClient::connect_uds(path).await,
    }
    .map_err(agent_error)?;
    let identities = agent.request_identities().await.map_err(agent_error)?;
    for identity in identities {
        let key = identity.public_key().into_owned();
        let hash = if key.algorithm().is_rsa() {
            rsa_hash(handle).await?
        } else {
            None
        };
        let result = handle
            .authenticate_publickey_with(user, key, hash, &mut agent)
            .await
            .map_err(|error| SshError::Agent {
                reason: error.to_string(),
            })?;
        let outcome = AuthOutcome::from(result);
        if outcome != AuthOutcome::Rejected {
            return Ok(outcome);
        }
    }
    Ok(AuthOutcome::Rejected)
}

/// Windows agents (Pageant, OpenSSH named pipe) are out of the spike's scope.
#[cfg(not(unix))]
pub(crate) async fn authenticate(
    _handle: &mut Handle<ClientHandler>,
    _user: &str,
    _source: &AgentSource,
) -> Result<AuthOutcome, SshError> {
    Err(SshError::AgentNotConfigured)
}

#[cfg(unix)]
fn agent_error(error: russh::keys::Error) -> SshError {
    match error {
        russh::keys::Error::EnvVar(_) => SshError::AgentNotConfigured,
        other => SshError::Agent {
            reason: other.to_string(),
        },
    }
}

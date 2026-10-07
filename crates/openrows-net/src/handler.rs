// SPDX-License-Identifier: Apache-2.0
//! russh client handler: host-key verification against `known_hosts`, with the
//! first-use decision delegated to the UI through [`HostKeyPolicy`].

use std::sync::Arc;

use russh::keys::{PublicKey, PublicKeyOrCertificate};

use crate::{
    BoxFuture, SshError,
    blocking::off_runtime,
    known_hosts::{HostKeyStatus, KnownHosts, host_port},
    timeouts::PromptClock,
};

/// A host key seen for the first time, awaiting the user's decision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnknownHost {
    /// Host as dialled (the `known_hosts` lookup name).
    pub host: String,
    /// Port as dialled.
    pub port: u16,
    /// Key algorithm, e.g. `ssh-ed25519`.
    pub algorithm: String,
    /// `SHA256:…` fingerprint to show the user.
    pub fingerprint: String,
    /// Algorithms already recorded for this host; non-empty deserves a warning.
    pub other_algorithms: Vec<String>,
}

/// The user's answer to a first-use prompt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FirstUseDecision {
    /// Abort the connection.
    Reject,
    /// Connect this time without recording the key.
    AcceptOnce,
    /// Connect and append the key to the app-managed `known_hosts`.
    AcceptAndRemember,
}

/// Decides about unknown host keys (TOFU dialog). Changed and revoked keys
/// never reach the policy: they are a hard stop.
///
/// Time spent here is excluded from the handshake timeout.
pub trait HostKeyPolicy: Send + Sync {
    /// Decide whether to trust `host`'s key.
    fn on_unknown_host<'a>(&'a self, host: &'a UnknownHost) -> BoxFuture<'a, FirstUseDecision>;
}

/// Per-hop russh handler.
pub(crate) struct ClientHandler {
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) known_hosts: Arc<KnownHosts>,
    pub(crate) policy: Arc<dyn HostKeyPolicy>,
    pub(crate) clock: PromptClock,
}

impl ClientHandler {
    /// Run a `known_hosts` file operation off the async runtime.
    async fn with_store<T: Send + 'static>(
        &self,
        key: &PublicKey,
        operation: fn(&KnownHosts, &str, u16, &PublicKey) -> Result<T, SshError>,
    ) -> Result<T, SshError> {
        let (store, host, port, key) = (
            Arc::clone(&self.known_hosts),
            self.host.clone(),
            self.port,
            key.clone(),
        );
        off_runtime(move || operation(&store, &host, port, &key)).await
    }

    async fn verify(&self, key: &PublicKey) -> Result<bool, SshError> {
        let status = self.with_store(key, KnownHosts::verify).await?;
        let host = host_port(&self.host, self.port);
        match status {
            HostKeyStatus::Trusted => Ok(true),
            HostKeyStatus::Changed { fingerprint, at } => Err(SshError::HostKeyChanged {
                host,
                fingerprint,
                at,
            }),
            HostKeyStatus::Revoked { fingerprint, at } => Err(SshError::HostKeyRevoked {
                host,
                fingerprint,
                at,
            }),
            HostKeyStatus::Unknown {
                fingerprint,
                other_algorithms,
            } => self.first_use(key, fingerprint, other_algorithms).await,
        }
    }

    async fn first_use(
        &self,
        key: &PublicKey,
        fingerprint: String,
        other_algorithms: Vec<String>,
    ) -> Result<bool, SshError> {
        let request = UnknownHost {
            host: self.host.clone(),
            port: self.port,
            algorithm: key.algorithm().to_string(),
            fingerprint,
            other_algorithms,
        };
        let decision = {
            let _pause = self.clock.pause();
            self.policy.on_unknown_host(&request).await
        };
        match decision {
            FirstUseDecision::Reject => Err(SshError::HostKeyRejected {
                host: host_port(&self.host, self.port),
                fingerprint: request.fingerprint,
            }),
            FirstUseDecision::AcceptOnce => Ok(true),
            FirstUseDecision::AcceptAndRemember => {
                self.with_store(key, KnownHosts::remember).await?;
                Ok(true)
            }
        }
    }
}

impl russh::client::Handler for ClientHandler {
    type Error = SshError;

    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKeyOrCertificate,
    ) -> Result<bool, SshError> {
        match server_public_key {
            PublicKeyOrCertificate::PublicKey { key, .. } => self.verify(key).await,
            PublicKeyOrCertificate::Certificate(_) => Err(SshError::HostCertificateUnsupported {
                host: host_port(&self.host, self.port),
            }),
        }
    }
}

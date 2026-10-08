// SPDX-License-Identifier: Apache-2.0
//! SSH tunnels for saved connections (D-10): one shared tunnel per profile
//! for its metadata and user sessions (docs/24 §3), opened on first use with
//! prompts in the webview, and the typed errors they produce.
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use openrows_net::config::{JumpSpec, SshConfig};
use openrows_net::known_hosts::KnownHosts;
use openrows_net::{
    HopTiming, PlanContext, SecretString, SshAuth, SshError, SshHop, SshRouteSpec, SshTunnel,
    Timeouts, TunnelOptions, plan_route,
};
use openrows_secrets::SecretString as StoredSecret;
use openrows_store::profile::{SshAuthMethod, SshSettings};
use tokio::sync::Mutex as AsyncMutex;

use super::error::{IpcError, IpcErrorKind};
use super::prompts::{PromptBroker, UiHostKeyPolicy, UiPrompter};

/// Where SSH looks things up on this machine.
#[derive(Clone, Debug)]
pub(crate) struct SshEnvironment {
    /// Home directory (`~` in key paths, `~/.ssh/config`, `~/.ssh/known_hosts`).
    pub(crate) home: PathBuf,
    /// App-managed `known_hosts` that first-use acceptances are appended to.
    pub(crate) learned_hosts: PathBuf,
    /// Login user when neither the profile nor `~/.ssh/config` names one.
    pub(crate) local_user: String,
}

impl SshEnvironment {
    fn known_hosts(&self) -> KnownHosts {
        let ssh = self.home.join(".ssh");
        KnownHosts::new(
            vec![ssh.join("known_hosts"), ssh.join("known_hosts2")],
            self.learned_hosts.clone(),
        )
    }
}

/// Passwords and passphrases for one SSH route, from the keychain or form.
#[derive(Default)]
pub(crate) struct SshSecrets {
    pub(crate) password: Option<StoredSecret>,
    pub(crate) passphrase: Option<StoredSecret>,
}

/// The keychain's zeroizing string as the network layer's.
fn net_secret(secret: Option<StoredSecret>) -> Option<SecretString> {
    secret.map(|secret| SecretString::from(secret.expose()))
}

/// The planner's view of a profile's SSH settings.
///
/// # Errors
/// `Invalid` when a jump host is malformed.
pub(crate) fn route_spec(ssh: &SshSettings, secrets: SshSecrets) -> Result<SshRouteSpec, IpcError> {
    let auth = match &ssh.auth {
        SshAuthMethod::Password => SshAuth::Password(net_secret(secrets.password)),
        SshAuthMethod::PrivateKey { key_file } => SshAuth::PrivateKey {
            path: key_file.clone(),
            passphrase: net_secret(secrets.passphrase),
        },
        SshAuthMethod::Agent => SshAuth::Agent,
        SshAuthMethod::KeyboardInteractive => SshAuth::KeyboardInteractive,
    };
    let mut jump_hosts = Vec::new();
    for jump in &ssh.jump_hosts {
        let parsed = JumpSpec::parse_list(jump)
            .map_err(|reason| IpcError::invalid(format!("Jump host {jump}: {reason}")))?;
        jump_hosts.extend(parsed);
    }
    Ok(SshRouteSpec {
        host: ssh.host.clone(),
        port: ssh.port,
        user: ssh.user.clone(),
        auth,
        jump_hosts,
    })
}

/// Plans and opens tunnels with prompts in the webview.
pub(crate) struct TunnelOpener {
    pub(crate) environment: SshEnvironment,
    pub(crate) prompts: Arc<PromptBroker>,
}

/// A tunnel attempt: the planned hops, the hops that came up and the outcome.
pub(crate) struct TunnelAttempt {
    pub(crate) hops: Vec<SshHop>,
    pub(crate) completed: Vec<HopTiming>,
    pub(crate) result: Result<SshTunnel, SshError>,
}

impl TunnelOpener {
    /// Resolve `~/.ssh/config` and plan the hops for `spec`.
    ///
    /// # Errors
    /// `Ssh` for an unreadable config, an empty host or a `ProxyJump` loop.
    pub(crate) async fn plan(&self, spec: &SshRouteSpec) -> Result<Vec<SshHop>, IpcError> {
        let home = self.environment.home.clone();
        let config = tauri::async_runtime::spawn_blocking(move || {
            SshConfig::load(&home.join(".ssh").join("config"), &home)
        })
        .await
        .map_err(|error| IpcError::new(IpcErrorKind::Internal, error.to_string()))?
        .map_err(|error| IpcError::new(IpcErrorKind::Ssh, format!("~/.ssh/config: {error}")))?;
        let context = PlanContext {
            config: &config,
            home: &self.environment.home,
            default_user: &self.environment.local_user,
        };
        plan_route(spec, context)
            .map_err(|error| IpcError::new(IpcErrorKind::Ssh, error.to_string()))
    }

    /// Connect every hop of `hops`, recording how far it got.
    pub(crate) async fn open(&self, hops: Vec<SshHop>) -> TunnelAttempt {
        let options = TunnelOptions {
            known_hosts: Arc::new(self.environment.known_hosts()),
            host_key_policy: Arc::new(UiHostKeyPolicy(Arc::clone(&self.prompts))),
            prompter: Arc::new(UiPrompter(Arc::clone(&self.prompts))),
            timeouts: Timeouts::default(),
        };
        // Boxed: the handshake state machine is large and every connect awaits it.
        let (result, completed) = Box::pin(SshTunnel::connect_traced(&hops, &options)).await;
        TunnelAttempt {
            hops,
            completed,
            result,
        }
    }

    /// Plan and open in one go, for connections (not the test).
    ///
    /// # Errors
    /// The planning error or the typed [`ssh_error`].
    pub(crate) async fn connect(&self, spec: &SshRouteSpec) -> Result<SshTunnel, IpcError> {
        let hops = self.plan(spec).await?;
        self.open(hops)
            .await
            .result
            .map_err(|error| ssh_error(&error))
    }
}

type Slot = Arc<AsyncMutex<Option<Arc<SshTunnel>>>>;

/// One shared tunnel per saved connection.
#[derive(Default)]
pub(crate) struct Tunnels {
    slots: Mutex<HashMap<String, Slot>>,
}

impl Tunnels {
    /// The open tunnel of `profile`, or a new one from `open`. Concurrent
    /// callers wait for the same attempt, so prompts appear once.
    ///
    /// # Errors
    /// The error of `open`.
    pub(crate) async fn get_or_open<F>(
        &self,
        profile: &str,
        open: F,
    ) -> Result<Arc<SshTunnel>, IpcError>
    where
        F: Future<Output = Result<SshTunnel, IpcError>>,
    {
        let slot = {
            let mut slots = self.slots.lock().unwrap_or_else(PoisonError::into_inner);
            Arc::clone(slots.entry(profile.to_owned()).or_default())
        };
        let mut guard = slot.lock().await;
        if let Some(tunnel) = guard.as_ref().filter(|tunnel| !tunnel.is_closed()) {
            return Ok(Arc::clone(tunnel));
        }
        let tunnel = Arc::new(open.await?);
        *guard = Some(Arc::clone(&tunnel));
        Ok(tunnel)
    }

    /// Stop sharing `profile`'s tunnel (after disconnect or an edit); it closes
    /// once the last session using it is gone.
    pub(crate) fn forget(&self, profile: &str) {
        self.slots
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(profile);
    }
}

/// Typed, user-readable SSH failures. A changed or revoked host key is a
/// hard stop with an explanation; dismissed prompts are cancellations.
pub(crate) fn ssh_error(error: &SshError) -> IpcError {
    match error {
        SshError::HostKeyChanged {
            host,
            fingerprint,
            at,
        } => IpcError::new(
            IpcErrorKind::HostKey,
            format!(
                "The SSH host key of {host} has changed (now {fingerprint}). Someone may be \
                 intercepting the connection, or the server was reinstalled. OpenRows will not \
                 connect. If the change is expected, remove the old key at {at} and connect \
                 again to review the new one."
            ),
        ),
        SshError::HostKeyRevoked { host, at, .. } => IpcError::new(
            IpcErrorKind::HostKey,
            format!(
                "The SSH host key of {host} is marked as revoked ({at}). OpenRows will not connect."
            ),
        ),
        SshError::HostCertificateUnsupported { .. } => {
            IpcError::new(IpcErrorKind::HostKey, error.to_string())
        }
        SshError::HostKeyRejected { host, .. } => IpcError::new(
            IpcErrorKind::Cancelled,
            format!("The host key of {host} was not trusted, so the connection was cancelled."),
        ),
        SshError::PromptCancelled => {
            IpcError::new(IpcErrorKind::Cancelled, "SSH authentication was cancelled.")
        }
        other => IpcError::new(IpcErrorKind::Ssh, format!("SSH: {other}")),
    }
}

#[cfg(test)]
mod tests;

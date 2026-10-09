// SPDX-License-Identifier: Apache-2.0
//! SSH tunnels for saved connections (D-10): one shared tunnel per profile
//! for its metadata and user sessions (docs/24 §3), opened on first use with
//! prompts in the webview, re-established when it dropped (M1-06), and the
//! typed errors they produce.
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use openrows_net::config::{JumpSpec, SshConfig};
use openrows_net::known_hosts::KnownHosts;
use openrows_net::{
    HopTiming, PlanContext, ProxySpec, SecretString, SharedTunnel, SshAuth, SshError, SshHop,
    SshRouteSpec, SshTunnel, Timeouts, TunnelOptions, plan_route,
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

impl TunnelAttempt {
    /// The connected tunnel as a [`SharedTunnel`] that reconnects with `options`.
    ///
    /// # Errors
    /// The typed [`ssh_error`] of a failed attempt.
    pub(crate) fn shared(self, options: TunnelOptions) -> Result<SharedTunnel, IpcError> {
        let tunnel = self.result.map_err(|error| ssh_error(&error))?;
        Ok(SharedTunnel::with_tunnel(tunnel, self.hops, options))
    }
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

    /// Prompts in the webview, the user's `known_hosts` and `proxy` (if any)
    /// in front of the first hop.
    pub(crate) fn options(&self, proxy: Option<Arc<ProxySpec>>) -> TunnelOptions {
        TunnelOptions {
            known_hosts: Arc::new(self.environment.known_hosts()),
            host_key_policy: Arc::new(UiHostKeyPolicy(Arc::clone(&self.prompts))),
            prompter: Arc::new(UiPrompter(Arc::clone(&self.prompts))),
            timeouts: Timeouts::default(),
            proxy,
        }
    }

    /// Connect every hop of `hops`, recording how far it got (Test connection).
    pub(crate) async fn open(&self, hops: Vec<SshHop>, options: &TunnelOptions) -> TunnelAttempt {
        // Boxed: the handshake state machine is large and every connect awaits it.
        let (result, completed) = Box::pin(SshTunnel::connect_traced(&hops, options)).await;
        TunnelAttempt {
            hops,
            completed,
            result,
        }
    }

    /// Plan a profile's shared tunnel; it connects on first use.
    ///
    /// # Errors
    /// The planning error.
    pub(crate) async fn shared(
        &self,
        spec: &SshRouteSpec,
        proxy: Option<Arc<ProxySpec>>,
    ) -> Result<SharedTunnel, IpcError> {
        let hops = self.plan(spec).await?;
        Ok(SharedTunnel::new(hops, self.options(proxy)))
    }
}

type Slot = Arc<AsyncMutex<Option<Arc<SharedTunnel>>>>;

/// One shared tunnel per saved connection.
#[derive(Default)]
pub(crate) struct Tunnels {
    slots: Mutex<HashMap<String, Slot>>,
}

impl Tunnels {
    /// The connected tunnel of `profile`: the shared one (reconnected when
    /// its transport dropped), or a new one planned by `plan`. Concurrent
    /// callers wait for the same attempt, so prompts appear once.
    ///
    /// # Errors
    /// The planning error, or the typed SSH error of the connection; a
    /// dropped tunnel that cannot be re-established says so.
    pub(crate) async fn get_or_open<F>(
        &self,
        profile: &str,
        plan: F,
    ) -> Result<Arc<SharedTunnel>, IpcError>
    where
        F: Future<Output = Result<SharedTunnel, IpcError>>,
    {
        let slot = {
            let mut slots = self.lock();
            Arc::clone(slots.entry(profile.to_owned()).or_default())
        };
        let mut guard = slot.lock().await;
        if let Some(shared) = guard.as_ref() {
            shared
                .tunnel()
                .await
                .map_err(|error| reopen_error(&error))?;
            return Ok(Arc::clone(shared));
        }
        let shared = Arc::new(plan.await?);
        shared.tunnel().await.map_err(|error| ssh_error(&error))?;
        *guard = Some(Arc::clone(&shared));
        Ok(shared)
    }

    /// True when `profile`'s live tunnel accepted an unknown host key
    /// without saving it.
    pub(crate) fn unsaved_host_keys(&self, profile: &str) -> bool {
        let slot = self.lock().get(profile).cloned();
        let shared = slot.and_then(|slot| slot.try_lock().ok().and_then(|guard| guard.clone()));
        shared
            .and_then(|shared| shared.current())
            .is_some_and(|tunnel| !tunnel.unremembered_host_keys().is_empty())
    }

    /// Stop sharing `profile`'s tunnel (after disconnect or an edit); it closes
    /// once the last session using it is gone.
    pub(crate) fn forget(&self, profile: &str) {
        self.lock().remove(profile);
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Slot>> {
        self.slots.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// A shared tunnel dropped and reconnecting failed: keep the typed kind
/// (host key, cancelled) and say what happened.
fn reopen_error(error: &SshError) -> IpcError {
    let typed = ssh_error(error);
    IpcError::new(
        typed.kind,
        format!(
            "The SSH tunnel was closed and could not be re-established. {}",
            typed.message
        ),
    )
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

// SPDX-License-Identifier: Apache-2.0
//! Turn a connection profile's SSH settings into ordered [`SshHop`]s.
//!
//! The SSH host may be a `~/.ssh/config` alias: its `HostName`, `User`,
//! `Port`, `IdentityFile`, `IdentitiesOnly` and `ProxyJump` apply, and the
//! profile's explicit port, user and jump hosts override them (docs/24 §3).

use std::path::{Path, PathBuf};

use crate::{
    AgentSource, AuthMethod, RouteError, SecretString, SshHop,
    config::{JumpSpec, SshConfig, expand_tilde},
    route::expand_target,
};

/// How the user authenticates to the destination SSH server.
#[derive(Clone, Debug)]
pub enum SshAuth {
    /// A password from the keychain; keyboard-interactive follows for MFA.
    Password(Option<SecretString>),
    /// A private key file (`~` allowed), with its passphrase if encrypted.
    PrivateKey {
        /// Key file path as entered.
        path: PathBuf,
        /// Passphrase from the keychain.
        passphrase: Option<SecretString>,
    },
    /// ssh-agent, then the alias's `IdentityFile`s (OpenSSH's order).
    Agent,
    /// Keyboard-interactive only (passwords, OTP and other MFA prompts).
    KeyboardInteractive,
}

/// An SSH route as a connection profile describes it.
#[derive(Clone, Debug)]
pub struct SshRouteSpec {
    /// SSH server: host name, IP address or `~/.ssh/config` alias.
    pub host: String,
    /// Port; the alias's `Port` or 22 when `None`.
    pub port: Option<u16>,
    /// Login user; the alias's `User` or the local user when `None`.
    pub user: Option<String>,
    /// Destination authentication.
    pub auth: SshAuth,
    /// Jump hosts, first hop first; empty uses the alias's `ProxyJump`.
    pub jump_hosts: Vec<JumpSpec>,
}

/// Where the planner looks things up.
#[derive(Clone, Copy, Debug)]
pub struct PlanContext<'a> {
    /// Parsed `~/.ssh/config` (empty when there is none).
    pub config: &'a SshConfig,
    /// Home directory for `~` in key paths.
    pub home: &'a Path,
    /// Login user when neither the profile nor the config names one.
    pub default_user: &'a str,
}

/// Ordered hops, jump hosts first and the destination SSH server last.
///
/// Jump hosts authenticate the OpenSSH way (agent unless `IdentitiesOnly`,
/// their `IdentityFile`s, keyboard-interactive) and also try the profile's
/// private key, since bastion chains usually share one.
///
/// # Errors
/// [`RouteError`] for an empty host or a `ProxyJump` loop.
pub fn plan_route(
    spec: &SshRouteSpec,
    context: PlanContext<'_>,
) -> Result<Vec<SshHop>, RouteError> {
    let host = spec.host.trim();
    if host.is_empty() {
        return Err(RouteError::MissingHost);
    }
    let mut target = context.config.resolve(host);
    if let Some(port) = spec.port {
        target.port = port;
    }
    if let Some(user) = spec.user.as_ref().filter(|user| !user.trim().is_empty()) {
        target.user = Some(user.trim().to_owned());
    }
    if !spec.jump_hosts.is_empty() {
        target.proxy_jump.clone_from(&spec.jump_hosts);
    }
    let mut settings = expand_target(context.config, target)?;
    let Some(destination) = settings.pop() else {
        return Err(RouteError::MissingHost);
    };
    let shared_key = profile_key(&spec.auth, context.home);
    let mut hops: Vec<SshHop> = settings
        .iter()
        .map(|jump| {
            let mut hop = SshHop::from_settings(jump, context.default_user);
            if let Some(key) = &shared_key {
                let at = usize::from(!jump.identities_only).min(hop.auth.len());
                hop.auth.insert(at, key.clone());
            }
            hop
        })
        .collect();
    let mut last = SshHop::from_settings(&destination, context.default_user);
    last.auth = destination_methods(&spec.auth, last.auth, context.home);
    hops.push(last);
    Ok(hops)
}

/// The profile's key file as a method, for jump hosts.
fn profile_key(auth: &SshAuth, home: &Path) -> Option<AuthMethod> {
    match auth {
        SshAuth::PrivateKey { path, passphrase } => Some(AuthMethod::PrivateKey {
            path: key_path(path, home),
            passphrase: passphrase.clone(),
        }),
        SshAuth::Password(_) | SshAuth::Agent | SshAuth::KeyboardInteractive => None,
    }
}

/// Methods for the destination; `defaults` is OpenSSH's order for the alias.
fn destination_methods(auth: &SshAuth, defaults: Vec<AuthMethod>, home: &Path) -> Vec<AuthMethod> {
    match auth {
        SshAuth::Password(Some(password)) => vec![
            AuthMethod::Password(password.clone()),
            AuthMethod::KeyboardInteractive,
        ],
        SshAuth::Password(None) | SshAuth::KeyboardInteractive => {
            vec![AuthMethod::KeyboardInteractive]
        }
        SshAuth::PrivateKey { .. } => profile_key(auth, home)
            .into_iter()
            .chain([AuthMethod::KeyboardInteractive])
            .collect(),
        SshAuth::Agent if defaults.iter().any(is_agent) => defaults,
        SshAuth::Agent => std::iter::once(AuthMethod::Agent(AgentSource::Environment))
            .chain(defaults)
            .collect(),
    }
}

fn is_agent(method: &AuthMethod) -> bool {
    matches!(method, AuthMethod::Agent(_))
}

fn key_path(path: &Path, home: &Path) -> PathBuf {
    path.to_str()
        .map_or_else(|| path.to_path_buf(), |text| expand_tilde(text, home))
}

#[cfg(test)]
mod tests;

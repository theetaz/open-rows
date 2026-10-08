// SPDX-License-Identifier: Apache-2.0
//! Engine-agnostic route settings of a profile: TLS (docs/12 §4.1) and an
//! optional SSH tunnel (docs/12 §4.2). Paths and names only; passwords and
//! passphrases live in the keychain (see [`super::SecretSlot`]).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{ProfileError, TlsMode};

/// TLS settings. Profiles written before D-11 hold only `mode`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TlsSettings {
    /// Negotiation and verification mode.
    pub mode: TlsMode,
    /// PEM CA bundle; the only trust anchor when set (libpq `sslrootcert`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ca_file: Option<PathBuf>,
    /// PEM client certificate (libpq `sslcert`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_cert: Option<PathBuf>,
    /// PEM client key (libpq `sslkey`); its passphrase is a keychain item.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_key: Option<PathBuf>,
}

impl TlsSettings {
    /// `mode` without certificate files.
    #[must_use]
    pub fn new(mode: TlsMode) -> Self {
        Self {
            mode,
            ca_file: None,
            client_cert: None,
            client_key: None,
        }
    }

    /// True when the server certificate is not authenticated: every mode
    /// below `verify-ca`, except `require` with a CA file (libpq).
    #[must_use]
    pub fn skips_verification(&self) -> bool {
        match self.mode {
            TlsMode::VerifyCa | TlsMode::VerifyFull => false,
            TlsMode::Require => self.ca_file.is_none(),
            TlsMode::Disable | TlsMode::Allow | TlsMode::Prefer => true,
        }
    }
}

impl TlsSettings {
    /// True when TLS verification is skipped for a remote `host`: the case
    /// the UI flags with a persistent warning (docs/12 §4.1). Over SSH the
    /// host is as seen from the last hop.
    #[must_use]
    pub fn unverified_remote(&self, host: &str) -> bool {
        self.skips_verification() && !is_local_host(host)
    }
}

/// Loopback names, IP literals and Unix socket directories.
#[must_use]
pub fn is_local_host(host: &str) -> bool {
    matches!(host, "localhost" | "127.0.0.1" | "::1") || host.starts_with('/')
}

/// How the user authenticates to the SSH server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "method", rename_all = "camelCase")]
pub enum SshAuthMethod {
    /// Password from the keychain; keyboard-interactive follows for MFA.
    Password,
    /// A private key file; an encrypted key's passphrase is a keychain item.
    #[serde(rename_all = "camelCase")]
    PrivateKey {
        /// Key file path (`~` allowed).
        key_file: PathBuf,
    },
    /// ssh-agent (`SSH_AUTH_SOCK`), then the `~/.ssh/config` identity files.
    Agent,
    /// Keyboard-interactive prompts only (passwords, OTP, other MFA).
    KeyboardInteractive,
}

/// An SSH tunnel in front of the database connection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshSettings {
    /// SSH server: host name, IP address or `~/.ssh/config` alias.
    pub host: String,
    /// Port; the alias's `Port` or 22 when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    /// Login user; the alias's `User` or the local user when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    /// Authentication method.
    pub auth: SshAuthMethod,
    /// Jump hosts (`ProxyJump` syntax, `[user@]host[:port]`), first hop first;
    /// empty uses the alias's `ProxyJump`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub jump_hosts: Vec<String>,
}

impl SshSettings {
    /// Check required fields.
    ///
    /// # Errors
    /// [`ProfileError::Invalid`] naming the first bad field.
    pub fn validate(&self) -> Result<(), ProfileError> {
        let invalid = |field, reason| Err(ProfileError::Invalid { field, reason });
        if self.host.trim().is_empty() {
            return invalid("ssh.host", "must not be empty");
        }
        if self.port == Some(0) {
            return invalid("ssh.port", "must be 1-65535");
        }
        if let SshAuthMethod::PrivateKey { key_file } = &self.auth
            && key_file.as_os_str().is_empty()
        {
            return invalid("ssh.keyFile", "must not be empty");
        }
        if self.jump_hosts.iter().any(|jump| jump.trim().is_empty()) {
            return invalid("ssh.jumpHosts", "must not contain empty entries");
        }
        Ok(())
    }
}

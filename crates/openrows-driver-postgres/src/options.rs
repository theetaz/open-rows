// SPDX-License-Identifier: Apache-2.0
//! Connection options beyond host and credentials: TLS mode and files, the
//! route, timeouts, startup SQL.
use std::{fmt, path::PathBuf, sync::Arc, time::Duration};

use openrows_net::{
    Dialer, SecretString,
    tls::{TlsParams, Verification},
};

/// libpq `sslmode`, with libpq's semantics (docs/12 §4.1).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SslMode {
    /// Never use TLS.
    Disable,
    /// Plaintext first; TLS only when the server rejects the plaintext session.
    Allow,
    /// TLS when the server offers it, else plaintext. Certificates are not checked.
    #[default]
    Prefer,
    /// TLS required; the certificate is checked only against a configured CA file.
    Require,
    /// TLS with the chain checked against the CA file (no host-name check).
    VerifyCa,
    /// TLS with chain and host-name checks; system trust store without a CA file.
    VerifyFull,
}

impl SslMode {
    /// The libpq spelling, e.g. `verify-full`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disable => "disable",
            Self::Allow => "allow",
            Self::Prefer => "prefer",
            Self::Require => "require",
            Self::VerifyCa => "verify-ca",
            Self::VerifyFull => "verify-full",
        }
    }

    /// True when the mode cannot be satisfied without TLS.
    #[must_use]
    pub fn requires_tls(self) -> bool {
        matches!(self, Self::Require | Self::VerifyCa | Self::VerifyFull)
    }

    /// The certificate check libpq performs: `require` with a CA file
    /// behaves like `verify-ca`; `allow` and `prefer` never verify.
    #[must_use]
    pub fn verification(self, has_ca_file: bool) -> Verification {
        match self {
            Self::Require if has_ca_file => Verification::Chain,
            Self::Disable | Self::Allow | Self::Prefer | Self::Require => Verification::None,
            Self::VerifyCa => Verification::Chain,
            Self::VerifyFull => Verification::Full,
        }
    }
}

/// Certificate files for TLS. `Debug` redacts the key passphrase.
#[derive(Clone, Debug, Default)]
pub struct TlsFiles {
    /// PEM CA bundle; the only trust anchor when set (libpq `sslrootcert`).
    pub ca_file: Option<PathBuf>,
    /// PEM client certificate (libpq `sslcert`).
    pub client_cert: Option<PathBuf>,
    /// PEM client key (libpq `sslkey`), optionally encrypted PKCS#8.
    pub client_key: Option<PathBuf>,
    /// Passphrase of an encrypted client key (libpq `sslpassword`).
    pub key_passphrase: Option<SecretString>,
}

impl TlsFiles {
    /// TLS parameters for `mode`.
    #[must_use]
    pub fn params(&self, mode: SslMode) -> TlsParams {
        TlsParams {
            verification: mode.verification(self.ca_file.is_some()),
            ca_file: self.ca_file.clone(),
            client_cert: self.client_cert.clone(),
            client_key: self.client_key.clone(),
            key_passphrase: self.key_passphrase.clone(),
        }
    }
}

/// Per-stage connect timeouts; defaults from docs/14 §4.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConnectTimeouts {
    /// Host name resolution.
    pub dns: Duration,
    /// TCP (or Unix socket, or routed stream) connect, across all resolved addresses.
    pub tcp: Duration,
    /// TLS negotiation and handshake.
    pub tls: Duration,
    /// Startup and authentication up to the first `ReadyForQuery`, plus the
    /// server-info query.
    pub auth: Duration,
}

impl Default for ConnectTimeouts {
    fn default() -> Self {
        Self {
            dns: Duration::from_secs(5),
            tcp: Duration::from_secs(10),
            tls: Duration::from_secs(10),
            auth: Duration::from_secs(15),
        }
    }
}

/// Everything about a connection that is not the target or the credentials.
#[derive(Clone)]
pub struct ConnectOptions {
    /// `application_name` reported to the server (visible in `pg_stat_activity`).
    pub application_name: String,
    /// TLS negotiation mode.
    pub ssl_mode: SslMode,
    /// CA and client certificate files.
    pub tls: TlsFiles,
    /// Streams come from this route (e.g. an SSH tunnel) instead of a direct
    /// TCP connection; cancel requests take the same route. The host name is
    /// still used for TLS verification.
    pub via: Option<Arc<dyn Dialer>>,
    /// Per-stage timeouts.
    pub timeouts: ConnectTimeouts,
    /// Statements run in order after connecting (FR-CONN-10). Each entry is sent
    /// as one simple query, so it may hold several `;`-separated statements.
    pub startup_commands: Vec<String>,
}

impl Default for ConnectOptions {
    fn default() -> Self {
        Self {
            application_name: "openrows".to_owned(),
            ssl_mode: SslMode::default(),
            tls: TlsFiles::default(),
            via: None,
            timeouts: ConnectTimeouts::default(),
            startup_commands: Vec::new(),
        }
    }
}

impl fmt::Debug for ConnectOptions {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConnectOptions")
            .field("application_name", &self.application_name)
            .field("ssl_mode", &self.ssl_mode)
            .field("tls", &self.tls)
            .field("via", &self.via.as_ref().map(|via| via.describe()))
            .field("timeouts", &self.timeouts)
            .field("startup_commands", &self.startup_commands.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verification_follows_libpq() {
        let cases = [
            (SslMode::Prefer, false, Verification::None),
            (SslMode::Require, false, Verification::None),
            (SslMode::Require, true, Verification::Chain),
            (SslMode::VerifyCa, true, Verification::Chain),
            (SslMode::VerifyFull, false, Verification::Full),
            (SslMode::VerifyFull, true, Verification::Full),
        ];
        for (mode, has_ca, expected) in cases {
            assert_eq!(mode.verification(has_ca), expected, "{mode:?} ca={has_ca}");
        }
    }

    #[test]
    fn debug_never_shows_the_key_passphrase() {
        let options = ConnectOptions {
            tls: TlsFiles {
                key_passphrase: Some(SecretString::from("hunter2-synthetic")),
                ..TlsFiles::default()
            },
            ..ConnectOptions::default()
        };
        assert!(!format!("{options:?}").contains("hunter2"));
    }
}

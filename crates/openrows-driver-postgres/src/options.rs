// SPDX-License-Identifier: Apache-2.0
//! Connection options beyond host and credentials: TLS mode, timeouts, startup SQL.
use std::time::Duration;

/// libpq `sslmode`. TLS itself arrives with D-11 (M1-06).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SslMode {
    /// Never use TLS.
    Disable,
    /// Plaintext first; TLS only if the server insists. Connects in plaintext.
    Allow,
    /// TLS when available, else plaintext. Connects in plaintext until D-11 and
    /// reports [`TlsStatus::PlaintextFallback`](crate::TlsStatus::PlaintextFallback).
    #[default]
    Prefer,
    /// TLS without certificate checks; rejected with `Unsupported` until D-11.
    Require,
    /// TLS with CA verification; rejected with `Unsupported` until D-11.
    VerifyCa,
    /// TLS with CA and host-name verification; rejected with `Unsupported` until D-11.
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
}

/// Per-stage connect timeouts; defaults from docs/14 §4.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConnectTimeouts {
    /// Host name resolution.
    pub dns: Duration,
    /// TCP (or Unix socket) connect, across all resolved addresses.
    pub tcp: Duration,
    /// Startup and authentication up to the first `ReadyForQuery`, plus the
    /// server-info query.
    pub auth: Duration,
}

impl Default for ConnectTimeouts {
    fn default() -> Self {
        Self {
            dns: Duration::from_secs(5),
            tcp: Duration::from_secs(10),
            auth: Duration::from_secs(15),
        }
    }
}

/// Everything about a connection that is not the target or the credentials.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConnectOptions {
    /// `application_name` reported to the server (visible in `pg_stat_activity`).
    pub application_name: String,
    /// TLS negotiation mode.
    pub ssl_mode: SslMode,
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
            timeouts: ConnectTimeouts::default(),
            startup_commands: Vec::new(),
        }
    }
}

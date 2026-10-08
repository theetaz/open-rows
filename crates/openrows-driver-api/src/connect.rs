// SPDX-License-Identifier: Apache-2.0
//! What a driver receives to open a connection (docs/05 §2): the endpoint,
//! credentials, TLS and route. The app resolves routes before a driver sees
//! the request: an SSH tunnel arrives as a [`Dialer`] in `via`, so drivers
//! never handle SSH themselves.
use std::{fmt, path::PathBuf, sync::Arc, time::Duration};

use openrows_net::{
    Dialer, SecretString,
    tls::{TlsParams, Verification},
};

/// libpq `sslmode`, with libpq's semantics (docs/12 §4.1). `MySQL` `ssl-mode`
/// maps onto it: `DISABLED`, `PREFERRED`, `REQUIRED`, `VERIFY_CA`, `VERIFY_IDENTITY`.
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

/// Where the database lives.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Endpoint {
    /// A network server: host name, IP literal or socket directory, and port.
    Server {
        /// Host as the profile names it (also used for TLS verification).
        host: String,
        /// TCP port.
        port: u16,
    },
    /// A local database file.
    File {
        /// Absolute file path.
        path: PathBuf,
        /// Open read-only: the engine refuses every write.
        read_only: bool,
        /// Create the file when it does not exist.
        create: bool,
    },
}

/// Everything a driver needs to open one connection. `Debug` redacts secrets.
#[derive(Clone)]
pub struct ConnectRequest {
    /// Server or file.
    pub endpoint: Endpoint,
    /// Database to open; empty for engines without databases.
    pub database: String,
    /// Login user; empty for file engines.
    pub user: String,
    /// Password, read from the keychain on the connect path only.
    pub password: Option<SecretString>,
    /// Name reported to the server (`application_name`, `program_name`).
    pub application_name: String,
    /// TLS negotiation mode.
    pub ssl_mode: SslMode,
    /// CA and client certificate files.
    pub tls: TlsFiles,
    /// Streams come from this route (an SSH tunnel) instead of direct TCP.
    pub via: Option<Arc<dyn Dialer>>,
    /// Per-stage timeouts.
    pub timeouts: ConnectTimeouts,
}

impl ConnectRequest {
    /// The same request for another database on the same server.
    #[must_use]
    pub fn with_database(&self, database: &str) -> Self {
        let mut request = self.clone();
        database.clone_into(&mut request.database);
        request
    }

    /// The same request reported to the server as `<name> <suffix>`.
    #[must_use]
    pub fn with_application_suffix(mut self, suffix: &str) -> Self {
        self.application_name = format!("{} {suffix}", self.application_name);
        self
    }

    /// Connect through `route` (an SSH tunnel) instead of direct TCP.
    #[must_use]
    pub fn through(mut self, route: Arc<dyn Dialer>) -> Self {
        self.via = Some(route);
        self
    }

    /// `host:port` or the file path, for messages.
    #[must_use]
    pub fn describe_endpoint(&self) -> String {
        match &self.endpoint {
            Endpoint::Server { host, port } => format!("{host}:{port}"),
            Endpoint::File { path, .. } => path.display().to_string(),
        }
    }
}

impl fmt::Debug for ConnectRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConnectRequest")
            .field("endpoint", &self.endpoint)
            .field("database", &self.database)
            .field("user", &self.user)
            .field("password", &self.password.as_ref().map(|_| "***"))
            .field("application_name", &self.application_name)
            .field("ssl_mode", &self.ssl_mode)
            .field("tls", &self.tls)
            .field("via", &self.via.as_ref().map(|via| via.describe()))
            .field("timeouts", &self.timeouts)
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
    fn debug_never_shows_secrets() {
        let request = ConnectRequest {
            endpoint: Endpoint::Server {
                host: "db.test".to_owned(),
                port: 3306,
            },
            database: "shop".to_owned(),
            user: "app".to_owned(),
            password: Some(SecretString::from("pw-synthetic")),
            application_name: "OpenRows".to_owned(),
            ssl_mode: SslMode::Prefer,
            tls: TlsFiles {
                key_passphrase: Some(SecretString::from("hunter2-synthetic")),
                ..TlsFiles::default()
            },
            via: None,
            timeouts: ConnectTimeouts::default(),
        };
        let text = format!("{request:?}");
        assert!(!text.contains("pw-synthetic") && !text.contains("hunter2"));
        let other = request
            .with_database("reports")
            .with_application_suffix("metadata");
        assert_eq!(other.database, "reports");
        assert_eq!(other.application_name, "OpenRows metadata");
        assert_eq!(other.describe_endpoint(), "db.test:3306");
    }
}

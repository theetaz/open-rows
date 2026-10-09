// SPDX-License-Identifier: Apache-2.0
//! Where and how a tool connects: the server address (possibly the loopback
//! end of the app's tunnel), the login and the profile's TLS settings.
use std::path::PathBuf;

/// libpq `sslmode` values; `MySQL` and `MariaDB` clients get the nearest flags.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TlsMode {
    /// No TLS.
    Disable,
    /// Plaintext first, TLS if the server insists.
    Allow,
    /// TLS when offered; certificates are not checked.
    #[default]
    Prefer,
    /// TLS required; checked only against a configured CA.
    Require,
    /// TLS with the chain checked against the CA.
    VerifyCa,
    /// TLS with chain and host-name checks.
    VerifyFull,
}

impl TlsMode {
    /// The libpq `sslmode` keyword.
    #[must_use]
    pub fn libpq(self) -> &'static str {
        match self {
            Self::Disable => "disable",
            Self::Allow => "allow",
            Self::Prefer => "prefer",
            Self::Require => "require",
            Self::VerifyCa => "verify-ca",
            Self::VerifyFull => "verify-full",
        }
    }

    /// Whether the server certificate chain is checked.
    #[must_use]
    pub fn verifies(self) -> bool {
        matches!(self, Self::VerifyCa | Self::VerifyFull)
    }
}

/// The profile's TLS settings as files the tools read.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct TlsSpec {
    /// Negotiation and verification level.
    pub mode: TlsMode,
    /// PEM CA bundle (`sslrootcert`, `--ssl-ca`).
    pub ca_file: Option<PathBuf>,
    /// PEM client certificate.
    pub client_cert: Option<PathBuf>,
    /// PEM client key (unencrypted: the tools cannot be given a passphrase safely).
    pub client_key: Option<PathBuf>,
}

/// Where the tool connects.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Address {
    /// Straight to the server.
    Tcp {
        /// Host name or address.
        host: String,
        /// Port.
        port: u16,
    },
    /// A local Unix socket (libpq: the socket directory or file; `MySQL`:
    /// the socket file).
    Socket(String),
    /// The app's loopback listener on `127.0.0.1:port`, which forwards to
    /// the server through the profile's SSH tunnel or proxy; `server_host`
    /// is the host the certificate is checked against.
    Forwarded {
        /// Local port of the listener.
        port: u16,
        /// The database host as the profile names it.
        server_host: String,
    },
}

/// Everything a tool needs to log in, except the password (which only
/// travels in a 0600 file).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ConnectionSpec {
    /// Where to connect.
    pub address: Address,
    /// Login user.
    pub user: String,
    /// Database to back up or restore into.
    pub database: String,
    /// TLS settings.
    pub tls: TlsSpec,
    /// Name reported to the server.
    pub application_name: String,
    /// The 0600 file holding the password (`PGPASSFILE`, `MySQL`
    /// `--defaults-extra-file`); `None` without a password.
    pub password_file: Option<PathBuf>,
}

impl ConnectionSpec {
    /// The same login on another database.
    #[must_use]
    pub fn with_database(&self, database: &str) -> Self {
        Self {
            database: database.to_owned(),
            ..self.clone()
        }
    }
}

/// libpq's socket file name prefix.
const PG_SOCKET_PREFIX: &str = ".s.PGSQL.";

/// `/tmp/.s.PGSQL.5433` → (`/tmp`, `Some(5433)`); a directory stays as is.
pub(crate) fn pg_socket_dir(path: &str) -> (String, Option<u16>) {
    let trimmed = path.trim_end_matches('/');
    match trimmed.rsplit_once('/') {
        Some((dir, file)) if file.starts_with(PG_SOCKET_PREFIX) => {
            let port = file[PG_SOCKET_PREFIX.len()..].parse().ok();
            let dir = if dir.is_empty() { "/" } else { dir };
            (dir.to_owned(), port)
        }
        _ => (trimmed.to_owned(), None),
    }
}

/// A libpq connection-string value: quoted, with `\` and `'` escaped.
pub(crate) fn conninfo_value(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('\'', "\\'");
    format!("'{escaped}'")
}

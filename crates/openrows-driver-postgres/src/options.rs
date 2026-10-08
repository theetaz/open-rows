// SPDX-License-Identifier: Apache-2.0
//! Connection options beyond host and credentials: TLS mode and files, the
//! route, timeouts, startup SQL.
use std::{fmt, sync::Arc};

pub use openrows_driver_api::connect::{ConnectTimeouts, SslMode, TlsFiles};
use openrows_net::Dialer;

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
    use openrows_net::SecretString;

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

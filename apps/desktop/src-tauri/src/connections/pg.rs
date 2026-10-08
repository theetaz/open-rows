// SPDX-License-Identifier: Apache-2.0
//! The only call site of the `PostgreSQL` driver's connect API (adapt here when
//! the driver changes).

use openrows_driver_api::DriverError;
use std::sync::Arc;

use openrows_driver_postgres::{
    ConnectOptions, Flavour, PgConfig, PgSession, ServerInfo, SslMode, TlsFiles,
};
use openrows_net::{Dialer, SecretString as NetSecret};
use openrows_secrets::SecretString;
use openrows_store::profile::{PgParams, TlsMode, TlsSettings};
use serde::Serialize;
use specta::Type;

use super::error::IpcError;

/// `application_name` when the profile does not set one.
const APPLICATION_NAME: &str = "OpenRows";

/// Database a connection without a configured database connects to: the
/// maintenance database every `PostgreSQL` server creates, so the user can
/// list and choose databases from there.
const HOME_DATABASE: &str = "postgres";

/// Where and how to connect: target, credentials and connect options.
#[derive(Clone)]
pub(crate) struct Target {
    pub(crate) config: PgConfig,
    pub(crate) options: ConnectOptions,
}

impl Target {
    /// The same server and options, connected to `database` instead.
    pub(crate) fn with_database(&self, database: &str) -> Self {
        let mut target = self.clone();
        database.clone_into(&mut target.config.database);
        target
    }

    /// Connect through `route` (an SSH tunnel) instead of direct TCP.
    pub(crate) fn through(mut self, route: Arc<dyn Dialer>) -> Self {
        self.options.via = Some(route);
        self
    }

    /// The same target under another `application_name`.
    pub(crate) fn with_application_suffix(mut self, suffix: &str) -> Self {
        self.options.application_name = format!("{} {suffix}", self.options.application_name);
        self
    }
}

/// Secrets the driver needs, read from the keychain on the connect path only.
#[derive(Default)]
pub(crate) struct DriverSecrets {
    pub(crate) password: Option<SecretString>,
    pub(crate) tls_key_passphrase: Option<SecretString>,
}

/// Driver settings for `params` and `tls`; the route (SSH) is added by
/// [`Target::through`].
///
/// # Errors
/// `Invalid` when no user is set.
pub(crate) fn target(
    params: &PgParams,
    tls: &TlsSettings,
    secrets: &DriverSecrets,
) -> Result<Target, IpcError> {
    let user = params
        .user
        .clone()
        .ok_or_else(|| IpcError::invalid("User is required"))?;
    let config = PgConfig {
        host: params.host.clone(),
        port: params.port,
        database: params
            .database
            .clone()
            .unwrap_or_else(|| HOME_DATABASE.to_owned()),
        user,
        password: secrets
            .password
            .as_ref()
            .map(|secret| secret.expose().to_owned()),
    };
    let options = ConnectOptions {
        application_name: params
            .application_name
            .clone()
            .unwrap_or_else(|| APPLICATION_NAME.to_owned()),
        ssl_mode: ssl_mode(tls.mode),
        tls: tls_files(tls, secrets.tls_key_passphrase.as_ref()),
        // Profiles have no startup commands yet (FR-CONN-10 lands with session settings).
        startup_commands: Vec::new(),
        ..ConnectOptions::default()
    };
    Ok(Target { config, options })
}

fn tls_files(tls: &TlsSettings, passphrase: Option<&SecretString>) -> TlsFiles {
    TlsFiles {
        ca_file: tls.ca_file.clone(),
        client_cert: tls.client_cert.clone(),
        client_key: tls.client_key.clone(),
        key_passphrase: passphrase.map(|secret| NetSecret::from(secret.expose())),
    }
}

/// The store's TLS mode is libpq `sslmode`, 1:1.
fn ssl_mode(mode: TlsMode) -> SslMode {
    match mode {
        TlsMode::Disable => SslMode::Disable,
        TlsMode::Allow => SslMode::Allow,
        TlsMode::Prefer => SslMode::Prefer,
        TlsMode::Require => SslMode::Require,
        TlsMode::VerifyCa => SslMode::VerifyCa,
        TlsMode::VerifyFull => SslMode::VerifyFull,
    }
}

/// What the UI shows about the server after connecting.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ServerDto {
    /// `PostgreSQL` or `CockroachDB`.
    pub(crate) product: String,
    /// `server_version`, e.g. `17.2`.
    pub(crate) server_version: String,
    /// Hosting or extension flavour worth naming, e.g. `Neon`.
    pub(crate) flavour: Option<String>,
    /// The database the session is connected to.
    pub(crate) database: String,
}

impl ServerDto {
    /// `PostgreSQL 17.2`, with the flavour when there is one.
    pub(crate) fn version_label(&self) -> String {
        let product = format!("{} {}", self.product, self.server_version);
        match &self.flavour {
            Some(flavour) => format!("{product} · {flavour}"),
            None => product,
        }
    }
}

impl From<&ServerInfo> for ServerDto {
    fn from(info: &ServerInfo) -> Self {
        let flavour = match info.flavour {
            Flavour::Timescale => Some("TimescaleDB"),
            Flavour::Supabase => Some("Supabase"),
            Flavour::Neon => Some("Neon"),
            Flavour::Postgres | Flavour::CockroachDb => None,
        };
        Self {
            product: info.product().to_owned(),
            server_version: info.server_version.clone(),
            flavour: flavour.map(str::to_owned),
            database: info.database.clone(),
        }
    }
}

/// A connected session and what the server reported about itself.
pub(crate) struct Connected {
    pub(crate) session: PgSession,
    pub(crate) server: ServerDto,
}

/// Connect, authenticate and read the server info; the driver applies the
/// per-stage timeouts (docs/14 §4).
///
/// # Errors
/// The driver failure, with host:port added to network errors.
pub(crate) async fn connect(target: &Target) -> Result<Connected, DriverError> {
    let session = PgSession::connect_with(&target.config, &target.options)
        .await
        .map_err(|error| with_target(error, &target.config))?;
    Ok(Connected {
        server: ServerDto::from(session.server_info()),
        session,
    })
}

/// Name the host for transport failures, which otherwise only say "refused".
fn with_target(error: DriverError, config: &PgConfig) -> DriverError {
    match error {
        DriverError::Network(io) => DriverError::Network(std::io::Error::new(
            io.kind(),
            format!("{}:{}: {io}", config.host, config.port),
        )),
        other => other,
    }
}

#[cfg(test)]
mod tests;

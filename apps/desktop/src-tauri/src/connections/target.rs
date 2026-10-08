// SPDX-License-Identifier: Apache-2.0
//! The only call site of every driver's connect API: a profile becomes a
//! driver [`ConnectRequest`] for the engine's [`Driver`] (adapt here when
//! the contract changes).
use std::path::PathBuf;
use std::sync::Arc;

use openrows_driver_api::{
    ConnectRequest, ConnectTimeouts, ConnectTrace, Connection, Driver, DriverError, Endpoint,
    ServerSummary, SslMode, TlsFiles,
};
use openrows_net::{Dialer, SecretString as NetSecret};
use openrows_secrets::SecretString;
use openrows_store::profile::{ConnectionProfile, TlsMode, TlsSettings};
use serde::Serialize;
use specta::Type;

use super::drivers;
use super::error::IpcError;

/// Name reported to servers when the profile does not set one.
const APPLICATION_NAME: &str = "OpenRows";

/// Where and how to connect: the engine's driver and its request.
#[derive(Clone)]
pub(crate) struct Target {
    pub(crate) driver: &'static dyn Driver,
    pub(crate) request: ConnectRequest,
}

impl Target {
    /// The same server and options, connected to `database` instead. File
    /// engines have one database and ignore it.
    pub(crate) fn with_database(&self, database: &str) -> Self {
        Self {
            driver: self.driver,
            request: self.request.with_database(database),
        }
    }

    /// Connect through `route` (an SSH tunnel) instead of direct TCP.
    pub(crate) fn through(self, route: Arc<dyn Dialer>) -> Self {
        Self {
            driver: self.driver,
            request: self.request.through(route),
        }
    }

    /// The same target under another application name.
    pub(crate) fn with_application_suffix(self, suffix: &str) -> Self {
        Self {
            driver: self.driver,
            request: self.request.with_application_suffix(suffix),
        }
    }

    /// The database a connection without a configured one opens.
    pub(crate) fn database(&self) -> &str {
        &self.request.database
    }
}

/// Secrets the driver needs, read from the keychain on the connect path only.
#[derive(Default)]
pub(crate) struct DriverSecrets {
    pub(crate) password: Option<SecretString>,
    pub(crate) tls_key_passphrase: Option<SecretString>,
}

/// Driver target for `profile`; the route (SSH) is added by [`Target::through`].
///
/// # Errors
/// `Unsupported` for engines without a driver; `Invalid` when a server
/// profile has no user or a file profile no file.
pub(crate) fn target(
    profile: &ConnectionProfile,
    secrets: &DriverSecrets,
) -> Result<Target, IpcError> {
    let driver = drivers::driver(profile.driver)?;
    let params = &profile.params;
    let (endpoint, user) = if profile.driver.is_file() {
        let file = profile
            .file
            .as_ref()
            .ok_or_else(|| IpcError::invalid("Choose a database file"))?;
        let endpoint = Endpoint::File {
            path: PathBuf::from(&file.path),
            read_only: file.read_only,
            create: file.create,
        };
        (endpoint, String::new())
    } else {
        let user = params
            .user
            .clone()
            .ok_or_else(|| IpcError::invalid("User is required"))?;
        let endpoint = Endpoint::Server {
            host: params.host.clone(),
            port: params.port,
        };
        (endpoint, user)
    };
    let request = ConnectRequest {
        endpoint,
        database: params
            .database
            .clone()
            .unwrap_or_else(|| driver.home_database().to_owned()),
        user,
        password: secrets
            .password
            .as_ref()
            .map(|secret| NetSecret::from(secret.expose())),
        application_name: params
            .application_name
            .clone()
            .unwrap_or_else(|| APPLICATION_NAME.to_owned()),
        ssl_mode: ssl_mode(profile.tls.mode),
        tls: tls_files(&profile.tls, secrets.tls_key_passphrase.as_ref()),
        via: None,
        timeouts: ConnectTimeouts::default(),
    };
    Ok(Target { driver, request })
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
    /// `PostgreSQL`, `CockroachDB`, `MySQL`, `MariaDB` or `SQLite`.
    pub(crate) product: String,
    /// Server or library version, e.g. `17.2`.
    pub(crate) server_version: String,
    /// Hosting or compatible-product flavour worth naming, e.g. `Neon`.
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

impl From<&ServerSummary> for ServerDto {
    fn from(summary: &ServerSummary) -> Self {
        Self {
            product: summary.product.clone(),
            server_version: summary.version.clone(),
            flavour: summary.flavour.clone(),
            database: summary.database.clone(),
        }
    }
}

/// A connected session and what the server reported about itself.
pub(crate) struct Connected {
    pub(crate) connection: Box<dyn Connection>,
    pub(crate) server: ServerDto,
}

/// Connect, authenticate and read the server info; the driver applies the
/// per-stage timeouts (docs/14 §4).
///
/// # Errors
/// The driver failure, with the endpoint added to network errors.
pub(crate) async fn connect(target: &Target) -> Result<Connected, DriverError> {
    let (result, _) = connect_traced(target).await;
    let connection = result?;
    Ok(Connected {
        server: ServerDto::from(connection.server()),
        connection,
    })
}

/// As [`connect`], also returning the stages that ran (Test connection).
pub(crate) async fn connect_traced(
    target: &Target,
) -> (Result<Box<dyn Connection>, DriverError>, ConnectTrace) {
    let mut trace = ConnectTrace::default();
    let result = target
        .driver
        .connect(&target.request, &mut trace)
        .await
        .map_err(|error| with_endpoint(error, &target.request));
    (result, trace)
}

/// Name the endpoint for transport failures, which otherwise only say "refused".
fn with_endpoint(error: DriverError, request: &ConnectRequest) -> DriverError {
    match error {
        DriverError::Network(io) => DriverError::Network(std::io::Error::new(
            io.kind(),
            format!("{}: {io}", request.describe_endpoint()),
        )),
        other => other,
    }
}

#[cfg(test)]
mod tests;

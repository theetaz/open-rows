// SPDX-License-Identifier: Apache-2.0
//! The only call site of the `PostgreSQL` driver's connect API (adapt here when
//! the driver changes).
use std::time::{Duration, Instant};

use openrows_driver_api::DriverError;
use openrows_driver_postgres::{ConnectOptions, Flavour, PgConfig, PgSession, ServerInfo, SslMode};
use openrows_secrets::SecretString;
use openrows_store::profile::{PgParams, TlsMode};
use serde::Serialize;
use specta::Type;

use super::error::{IpcError, IpcErrorKind};

/// `application_name` when the profile does not set one.
const APPLICATION_NAME: &str = "OpenRows";

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

    /// The same target under another `application_name`.
    pub(crate) fn with_application_suffix(mut self, suffix: &str) -> Self {
        self.options.application_name = format!("{} {suffix}", self.options.application_name);
        self
    }
}

/// Driver settings for `params`.
///
/// # Errors
/// `Unsupported` for modes that demand TLS (the driver is plain TCP for now),
/// `Invalid` when no user is set.
pub(crate) fn target(
    params: &PgParams,
    tls: TlsMode,
    password: Option<&SecretString>,
) -> Result<Target, IpcError> {
    let ssl_mode = ssl_mode(tls);
    if ssl_mode.requires_tls() {
        return Err(IpcError::new(
            IpcErrorKind::Unsupported,
            "TLS is not supported yet; choose TLS mode Prefer or Disable to connect without TLS",
        ));
    }
    let user = params
        .user
        .clone()
        .ok_or_else(|| IpcError::invalid("User is required"))?;
    let config = PgConfig {
        host: params.host.clone(),
        port: params.port,
        database: params.database.clone().unwrap_or_else(|| user.clone()),
        user,
        password: password.map(|secret| secret.expose().to_owned()),
    };
    let options = ConnectOptions {
        application_name: params
            .application_name
            .clone()
            .unwrap_or_else(|| APPLICATION_NAME.to_owned()),
        ssl_mode,
        // Profiles have no startup commands yet (FR-CONN-10 lands with session settings).
        startup_commands: Vec::new(),
        ..ConnectOptions::default()
    };
    Ok(Target { config, options })
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
    pub(crate) elapsed: Duration,
}

/// Connect, authenticate and read the server info; the driver applies the
/// per-stage timeouts (docs/14 §4).
///
/// # Errors
/// The driver failure, with host:port added to network errors.
pub(crate) async fn connect(target: &Target) -> Result<Connected, DriverError> {
    let started = Instant::now();
    let session = PgSession::connect_with(&target.config, &target.options)
        .await
        .map_err(|error| with_target(error, &target.config))?;
    Ok(Connected {
        server: ServerDto::from(session.server_info()),
        session,
        elapsed: started.elapsed(),
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
mod tests {
    use super::*;
    use openrows_driver_postgres::TlsStatus;

    fn params(user: Option<&str>, database: Option<&str>) -> PgParams {
        PgParams {
            user: user.map(str::to_owned),
            database: database.map(str::to_owned),
            ..PgParams::default()
        }
    }

    #[test]
    fn database_defaults_to_the_user_like_libpq() -> Result<(), IpcError> {
        let secret = SecretString::from("synthetic");
        let target = target(&params(Some("app"), None), TlsMode::Prefer, Some(&secret))?;
        let config = &target.config;
        assert_eq!(
            (config.user.as_str(), config.database.as_str()),
            ("app", "app")
        );
        assert_eq!(config.password.as_deref(), Some("synthetic"));
        assert_eq!(target.options.application_name, "OpenRows");
        assert_eq!(target.options.ssl_mode, SslMode::Prefer);
        Ok(())
    }

    #[test]
    fn tls_only_modes_and_missing_users_are_rejected() {
        let tls = target(&params(Some("app"), None), TlsMode::VerifyFull, None);
        assert!(tls.is_err_and(|error| error.kind == IpcErrorKind::Unsupported));
        let user = target(&params(None, Some("db")), TlsMode::Disable, None);
        assert!(user.is_err_and(|error| error.message == "User is required"));
    }

    #[test]
    fn metadata_targets_switch_database_and_name_themselves() -> Result<(), IpcError> {
        let profile = PgParams {
            application_name: Some("billing".to_owned()),
            ..params(Some("app"), Some("main"))
        };
        let base = target(&profile, TlsMode::Disable, None)?;
        let metadata = base
            .with_database("reports")
            .with_application_suffix("metadata");
        assert_eq!(metadata.config.database, "reports");
        assert_eq!(metadata.options.application_name, "billing metadata");
        assert_eq!(metadata.options.ssl_mode, SslMode::Disable);
        Ok(())
    }

    #[test]
    fn server_info_names_product_version_and_flavour() {
        let info = ServerInfo {
            server_version: "17.2".to_owned(),
            version: "PostgreSQL 17.2".to_owned(),
            database: "orders".to_owned(),
            user: "app".to_owned(),
            flavour: Flavour::Neon,
            tls: TlsStatus::Disabled,
        };
        let server = ServerDto::from(&info);
        assert_eq!(server.product, "PostgreSQL");
        assert_eq!(server.flavour.as_deref(), Some("Neon"));
        assert_eq!(server.database, "orders");
    }

    #[test]
    fn network_errors_name_the_target() -> Result<(), IpcError> {
        let target = target(&params(Some("app"), None), TlsMode::Disable, None)?;
        let error = with_target(
            DriverError::Network(std::io::ErrorKind::ConnectionRefused.into()),
            &target.config,
        );
        assert!(error.to_string().contains("localhost:5432"));
        Ok(())
    }
}

// SPDX-License-Identifier: Apache-2.0
//! The only call site of the `PostgreSQL` driver's connect API (adapt here when
//! the driver changes).
use std::time::{Duration, Instant};

use arrow_array::cast::AsArray;
use arrow_array::{Array, RecordBatch};
use arrow_schema::SchemaRef;
use openrows_driver_api::{DriverError, ResultSink, Session, SinkFlow};
use openrows_driver_postgres::{PgConfig, PgSession};
use openrows_secrets::SecretString;
use openrows_store::profile::{PgParams, TlsMode};

use super::error::{IpcError, IpcErrorKind};

/// Upper bound for TCP connect, authentication and the version query.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Driver settings for `params`.
///
/// # Errors
/// `Unsupported` for modes that demand TLS (the driver is plain TCP for now),
/// `Invalid` when no user is set.
pub(crate) fn config(
    params: &PgParams,
    tls: TlsMode,
    password: Option<&SecretString>,
) -> Result<PgConfig, IpcError> {
    if matches!(
        tls,
        TlsMode::Require | TlsMode::VerifyCa | TlsMode::VerifyFull
    ) {
        return Err(IpcError::new(
            IpcErrorKind::Unsupported,
            "TLS is not supported yet; choose TLS mode Prefer or Disable to connect without TLS",
        ));
    }
    let user = params
        .user
        .clone()
        .ok_or_else(|| IpcError::invalid("User is required"))?;
    Ok(PgConfig {
        host: params.host.clone(),
        port: params.port,
        database: params.database.clone().unwrap_or_else(|| user.clone()),
        user,
        password: password.map(|secret| secret.expose().to_owned()),
    })
}

/// A connected session and the server's version string.
pub(crate) struct Connected {
    pub(crate) session: PgSession,
    pub(crate) server_version: String,
    pub(crate) elapsed: Duration,
}

/// Connect, authenticate and read `server_version` within [`CONNECT_TIMEOUT`].
///
/// # Errors
/// The driver failure; a timeout is reported as a network error.
pub(crate) async fn connect(config: PgConfig) -> Result<Connected, DriverError> {
    let started = Instant::now();
    let target = format!("{}:{}", config.host, config.port);
    let attempt = async {
        let mut session = PgSession::connect(&config).await?;
        let server_version = server_version(&mut session).await?;
        Ok::<_, DriverError>((session, server_version))
    };
    let (session, server_version) = tokio::time::timeout(CONNECT_TIMEOUT, attempt)
        .await
        .map_err(|_| timed_out(&target))?
        .map_err(|error| with_target(error, &target))?;
    Ok(Connected {
        session,
        server_version,
        elapsed: started.elapsed(),
    })
}

async fn server_version(session: &mut PgSession) -> Result<String, DriverError> {
    let mut sink = FirstText::default();
    session.execute("SHOW server_version", &mut sink).await?;
    Ok(sink.0.unwrap_or_default())
}

fn timed_out(target: &str) -> DriverError {
    DriverError::Network(std::io::Error::new(
        std::io::ErrorKind::TimedOut,
        format!(
            "{target} did not answer within {} s",
            CONNECT_TIMEOUT.as_secs()
        ),
    ))
}

/// Name the host for transport failures, which otherwise only say "refused".
fn with_target(error: DriverError, target: &str) -> DriverError {
    match error {
        DriverError::Network(io) => {
            DriverError::Network(std::io::Error::new(io.kind(), format!("{target}: {io}")))
        }
        other => other,
    }
}

/// Keeps the first text cell of the first result.
#[derive(Default)]
struct FirstText(Option<String>);

impl ResultSink for FirstText {
    fn begin_result(&mut self, _schema: SchemaRef) {}

    fn push_batch(&mut self, batch: RecordBatch) -> SinkFlow {
        if self.0.is_none() && batch.num_rows() > 0 {
            self.0 = batch
                .columns()
                .first()
                .and_then(|column| column.as_string_opt::<i32>())
                .filter(|text| text.is_valid(0))
                .map(|text| text.value(0).to_owned());
        }
        SinkFlow::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let config = config(&params(Some("app"), None), TlsMode::Prefer, Some(&secret))?;
        assert_eq!(
            (config.user.as_str(), config.database.as_str()),
            ("app", "app")
        );
        assert_eq!(config.password.as_deref(), Some("synthetic"));
        Ok(())
    }

    #[test]
    fn tls_only_modes_and_missing_users_are_rejected() {
        let tls = config(&params(Some("app"), None), TlsMode::VerifyFull, None);
        assert!(tls.is_err_and(|error| error.kind == IpcErrorKind::Unsupported));
        let user = config(&params(None, Some("db")), TlsMode::Disable, None);
        assert!(user.is_err_and(|error| error.message == "User is required"));
    }

    #[test]
    fn network_errors_name_the_target() {
        let error = with_target(
            DriverError::Network(std::io::ErrorKind::ConnectionRefused.into()),
            "db.test:5432",
        );
        assert!(error.to_string().contains("db.test:5432"));
    }
}

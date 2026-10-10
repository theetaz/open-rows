// SPDX-License-Identifier: Apache-2.0
//! The remote libSQL driver and authenticated connection lifecycle.
use crate::{LibsqlSession, ResultLimits, http::Http};
use openrows_driver_api::{
    BoxFuture, ConnectRequest, ConnectStage, ConnectTrace, Connection, DiscardSink, Driver,
    DriverError, ServerSummary, Session, TlsStatus,
};
use openrows_sql::Dialect;

/// libSQL and Turso remote servers using Hrana 3 over HTTP.
#[derive(Clone, Copy, Debug, Default)]
pub struct LibsqlDriver;

impl Driver for LibsqlDriver {
    fn product(&self) -> &'static str {
        "libSQL / Turso"
    }
    fn dialect(&self) -> Dialect {
        Dialect::Sqlite
    }
    fn home_database(&self) -> &'static str {
        "main"
    }
    fn capabilities(&self) -> openrows_driver_api::Capabilities {
        crate::capabilities::CAPABILITIES
    }
    fn connect<'a>(
        &'a self,
        request: &'a ConnectRequest,
        trace: &'a mut ConnectTrace,
    ) -> BoxFuture<'a, Result<Box<dyn Connection>, DriverError>> {
        Box::pin(async move { Ok(Box::new(connect(request, trace).await?) as Box<dyn Connection>) })
    }
}

/// Authenticate, read `SQLite` server version, and run startup commands on one baton.
/// # Errors
/// Typed network, TLS, auth, protocol, SQL and option validation failures.
pub async fn connect(
    request: &ConnectRequest,
    trace: &mut ConnectTrace,
) -> Result<LibsqlSession, DriverError> {
    let limits = limits(request)?;
    let http = Http::new(request, trace).await?;
    let summary = ServerSummary {
        product: "libSQL / Turso".into(),
        version: String::new(),
        flavour: Some("Hrana 3 HTTP".into()),
        database: "main".into(),
        user: String::new(),
        tls: trace.tls.clone().unwrap_or(TlsStatus::Disabled),
    };
    let mut session = LibsqlSession::new(http, summary, limits);
    let mut sink = DiscardSink;
    let auth = session.execute("SELECT 1", &mut sink);
    if let Err(error) = trace.stage(ConnectStage::Authenticate, auth).await {
        let _ = Box::new(session).close().await;
        return Err(error);
    }
    let version = session.text_query("SELECT sqlite_version()", &[]);
    let rows = match trace.stage(ConnectStage::ServerInfo, version).await {
        Ok(rows) => rows,
        Err(error) => {
            let _ = Box::new(session).close().await;
            return Err(error);
        }
    };
    session.summary.version = rows
        .first()
        .and_then(|row| row.first())
        .and_then(Clone::clone)
        .ok_or_else(crate::error::protocol)?;
    if let Err(error) = trace
        .stage(
            ConnectStage::StartupCommands,
            startup(&mut session, &request.startup_commands),
        )
        .await
    {
        let _ = Box::new(session).close().await;
        return Err(error);
    }
    Ok(session)
}

async fn startup(session: &mut LibsqlSession, commands: &[String]) -> Result<(), DriverError> {
    for (index, sql) in commands.iter().enumerate() {
        session
            .control(sql, &[])
            .await
            .map_err(|error| error.in_startup_command(index))?;
    }
    Ok(())
}

fn limits(request: &ConnectRequest) -> Result<ResultLimits, DriverError> {
    let mut limits = ResultLimits::default();
    for (key, value) in &request.driver_options {
        match key.as_str() {
            "libsql_row_cap" => limits.rows = value.parse().map_err(|_| invalid())?,
            "libsql_response_bytes" => {
                limits.response_bytes = value.parse().map_err(|_| invalid())?;
            }
            "libsql_timeout_seconds" => {
                limits.timeout =
                    std::time::Duration::from_secs(value.parse().map_err(|_| invalid())?);
            }
            _ => {}
        }
    }
    limits.validate()
}
fn invalid() -> DriverError {
    DriverError::Unsupported("invalid libSQL result limit option".into())
}

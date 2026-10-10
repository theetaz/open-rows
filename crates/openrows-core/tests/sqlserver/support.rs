// SPDX-License-Identifier: Apache-2.0
//! Connection and assertion helpers for native showplan tests.
use openrows_core::{QueryEvent, SessionManager};
use openrows_driver_api::{
    ConnectRequest, ConnectTimeouts, ConnectTrace, Endpoint, Session, SslMode, TlsFiles,
};
use openrows_net::SecretString;
use openrows_types::SessionId;

/// An integration assertion or connection failure.
pub type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// Open the test container session through the actor.
pub async fn open(manager: &SessionManager) -> TestResult<SessionId> {
    let port = std::env::var("OPENROWS_TEST_MSSQL_PORT")?.parse()?;
    let password = std::env::var("OPENROWS_TEST_MSSQL_PASSWORD")?;
    let request = ConnectRequest {
        endpoint: Endpoint::Server {
            host: "127.0.0.1".to_owned(),
            port,
        },
        database: "openrows_test".to_owned(),
        user: "sa".to_owned(),
        password: Some(SecretString::from(password.as_str())),
        application_name: "openrows-explain-test".to_owned(),
        ssl_mode: SslMode::Require,
        tls: TlsFiles::default(),
        via: None,
        timeouts: ConnectTimeouts {
            auth: std::time::Duration::from_secs(60),
            ..ConnectTimeouts::default()
        },
        driver_options: std::collections::BTreeMap::new(),
        startup_commands: Vec::new(),
    };
    Ok(Box::pin(manager.open(async move {
        let connection =
            openrows_driver_mssql::connect(&request, &mut ConnectTrace::default()).await?;
        Ok(Box::new(connection) as Box<dyn Session>)
    }))
    .await?)
}

/// Execute a setup/assertion batch, propagating server errors.
pub async fn run(manager: &SessionManager, session: SessionId, sql: &str) -> TestResult {
    let mut result = manager.execute(session, sql)?;
    while let Some(event) = result.next_event().await {
        if let QueryEvent::Failed { error } = event {
            return Err(error.into());
        }
    }
    Ok(())
}

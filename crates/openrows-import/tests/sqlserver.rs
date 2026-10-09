// SPDX-License-Identifier: Apache-2.0
//! Imports into a real SQL Server through multi-row parameterised
//! `INSERT`s (`MERGE` for conflicts).
//!
//! Ignored by `just test`; `just test-mssql` starts a throwaway container
//! and runs it with `--run-ignored only`.
mod support;

use openrows_driver_api::{
    ConnectRequest, ConnectTimeouts, ConnectTrace, Endpoint, SslMode, TlsFiles,
};
use openrows_net::SecretString;
use openrows_sql::Dialect;
use support::scenarios::{Engine, all};

const DATABASE: &str = "openrows_test";

const ENGINE: Engine = Engine {
    dialect: Dialect::SqlServer,
    schema: "dbo",
    columns: "(id int PRIMARY KEY, name nvarchar(100), score float, active bit, joined date)",
};

fn request() -> support::TestResult<ConnectRequest> {
    let variable = |name: &str| {
        std::env::var(name).map_err(|_| format!("{name} is unset: run with `just test-mssql`"))
    };
    Ok(ConnectRequest {
        endpoint: Endpoint::Server {
            host: "127.0.0.1".to_owned(),
            port: variable("OPENROWS_TEST_MSSQL_PORT")?.parse()?,
        },
        database: DATABASE.to_owned(),
        user: "sa".to_owned(),
        password: Some(SecretString::from(
            variable("OPENROWS_TEST_MSSQL_PASSWORD")?.as_str(),
        )),
        application_name: "openrows-test".to_owned(),
        ssl_mode: SslMode::Require,
        tls: TlsFiles::default(),
        via: None,
        // The server may run emulated (amd64 on arm64): allow slow logins.
        timeouts: ConnectTimeouts {
            auth: std::time::Duration::from_secs(60),
            ..ConnectTimeouts::default()
        },
        startup_commands: Vec::new(),
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs SQL Server; run with `just test-mssql`"]
async fn every_import_scenario_passes_on_sql_server() -> support::TestResult {
    let mut connection =
        openrows_driver_mssql::connect(&request()?, &mut ConnectTrace::default()).await?;
    all(&mut connection, &ENGINE).await
}

// SPDX-License-Identifier: Apache-2.0
//! Imports into real `MySQL` and `MariaDB` servers through multi-row
//! parameterised `INSERT`s (`ON DUPLICATE KEY UPDATE` for conflicts).
//!
//! Ignored by `just test`; `just test-mysql` starts throwaway containers and
//! runs them with `--run-ignored only`.
mod support;

use openrows_driver_api::{
    ConnectRequest, ConnectTimeouts, ConnectTrace, Endpoint, SslMode, TlsFiles,
};
use openrows_net::SecretString;
use openrows_sql::Dialect;
use support::scenarios::{Engine, all};

const DATABASE: &str = "openrows_test";

const ENGINE: Engine = Engine {
    dialect: Dialect::MySql,
    schema: DATABASE,
    columns: "(id int PRIMARY KEY, name varchar(100), score double, active tinyint(1), joined date)",
};

fn request() -> support::TestResult<ConnectRequest> {
    let variable = |name: &str| {
        std::env::var(name).map_err(|_| format!("{name} is unset: run with `just test-mysql`"))
    };
    Ok(ConnectRequest {
        endpoint: Endpoint::Server {
            host: "127.0.0.1".to_owned(),
            port: variable("OPENROWS_TEST_MYSQL_PORT")?.parse()?,
        },
        database: DATABASE.to_owned(),
        user: "root".to_owned(),
        password: Some(SecretString::from(
            variable("OPENROWS_TEST_MYSQL_PASSWORD")?.as_str(),
        )),
        application_name: "openrows-test".to_owned(),
        ssl_mode: SslMode::Prefer,
        tls: TlsFiles::default(),
        via: None,
        timeouts: ConnectTimeouts::default(),
        startup_commands: Vec::new(),
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs MySQL or MariaDB; run with `just test-mysql`"]
async fn every_import_scenario_passes_on_mysql() -> support::TestResult {
    let mut connection =
        openrows_driver_mysql::connect(&request()?, &mut ConnectTrace::default()).await?;
    all(&mut connection, &ENGINE).await
}

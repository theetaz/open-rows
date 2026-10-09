// SPDX-License-Identifier: Apache-2.0
//! Schema cache happy path on a real `MySQL` or `MariaDB` server.
//!
//! Ignored by `just test`; `just test-mysql` starts throwaway containers and
//! runs it with `--run-ignored only`.
mod support;

use openrows_driver_api::{
    ConnectRequest, ConnectTimeouts, ConnectTrace, Endpoint, SslMode, TlsFiles, editor,
};
use openrows_net::SecretString;
use support::{Place, TestResult, happy_path};

const DATABASE: &str = "openrows_test";

fn request() -> TestResult<ConnectRequest> {
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

#[tokio::test]
#[ignore = "needs MySQL or MariaDB: run with `just test-mysql`"]
async fn mysql_listings_snapshot_search_and_refresh_after_ddl() -> TestResult {
    let mut session =
        openrows_driver_mysql::connect(&request()?, &mut ConnectTrace::default()).await?;
    editor(&mut session)?
        .commit_ddl(&[
            "DROP TABLE IF EXISTS cache_invoices".to_owned(),
            "CREATE TABLE cache_invoices (id int PRIMARY KEY)".to_owned(),
        ])
        .await?;
    let place = Place {
        database: DATABASE,
        schema: DATABASE,
        table: "cache_invoices",
    };
    happy_path(&mut session, &place).await
}

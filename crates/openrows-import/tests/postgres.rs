// SPDX-License-Identifier: Apache-2.0
//! Imports into a real `PostgreSQL` server through `COPY FROM STDIN` and
//! batched `INSERT … ON CONFLICT`.
//!
//! Ignored by `just test`; `just test-pg [version]` starts a throwaway
//! container and runs them with `--run-ignored only`.
mod support;

use openrows_driver_postgres::{PgConfig, PgSession};
use openrows_sql::Dialect;
use support::scenarios::{Engine, all};

const ENGINE: Engine = Engine {
    dialect: Dialect::Postgres,
    schema: "public",
    columns: "(id integer PRIMARY KEY, name text, score numeric, active boolean, joined date)",
};

fn config() -> support::TestResult<PgConfig> {
    let variable = |name: &str| {
        std::env::var(name)
            .map_err(|_| format!("{name} is unset: run these ignored tests with `just test-pg`"))
    };
    Ok(PgConfig {
        host: "127.0.0.1".to_owned(),
        port: variable("OPENROWS_TEST_PG_PORT")?.parse()?,
        user: "postgres".to_owned(),
        password: Some(variable("OPENROWS_TEST_PG_PASSWORD")?),
        database: "postgres".to_owned(),
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs PostgreSQL; run with `just test-pg`"]
async fn every_import_scenario_passes_on_postgres() -> support::TestResult {
    let mut connection = PgSession::connect(&config()?).await?;
    all(&mut connection, &ENGINE).await
}

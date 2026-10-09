// SPDX-License-Identifier: Apache-2.0
//! Schema cache happy path on a real `PostgreSQL` server.
//!
//! Ignored by `just test`; `just test-pg` starts a throwaway container and
//! runs it with `--run-ignored only`.
mod support;

use openrows_driver_api::editor;
use openrows_driver_postgres::{PgConfig, PgSession};
use support::{Place, TestResult, happy_path};

fn config() -> TestResult<PgConfig> {
    let variable = |name: &str| {
        std::env::var(name).map_err(|_| format!("{name} is unset: run with `just test-pg`"))
    };
    Ok(PgConfig {
        host: "127.0.0.1".to_owned(),
        port: variable("OPENROWS_TEST_PG_PORT")?.parse()?,
        user: "postgres".to_owned(),
        password: Some(variable("OPENROWS_TEST_PG_PASSWORD")?),
        database: "postgres".to_owned(),
    })
}

#[tokio::test]
#[ignore = "needs PostgreSQL: run with `just test-pg`"]
async fn postgres_listings_snapshot_search_and_refresh_after_ddl() -> TestResult {
    let mut session = PgSession::connect(&config()?).await?;
    editor(&mut session)?
        .commit_ddl(&[
            "DROP TABLE IF EXISTS cache_invoices".to_owned(),
            "CREATE TABLE cache_invoices (id integer PRIMARY KEY)".to_owned(),
        ])
        .await?;
    let place = Place {
        database: "postgres",
        schema: "public",
        table: "cache_invoices",
    };
    happy_path(&mut session, &place).await
}

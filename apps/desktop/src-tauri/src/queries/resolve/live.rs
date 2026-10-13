// SPDX-License-Identifier: Apache-2.0
use super::*;
use openrows_driver_postgres::{PgConfig, PgSession};

#[tokio::test]
#[ignore = "requires a disposable PostgreSQL server via OPENROWS_TEST_PG_PORT"]
async fn postgres_search_path_temp_views_and_quoted_names() -> TestResult {
    let config = PgConfig {
        host: "127.0.0.1".into(),
        port: std::env::var("OPENROWS_TEST_PG_PORT")?.parse()?,
        user: "postgres".into(),
        password: std::env::var("OPENROWS_TEST_PG_PASSWORD").ok(),
        database: "postgres".into(),
    };
    let mut connection = PgSession::connect(&config).await?;
    connection.execute("BEGIN", &mut DiscardSink).await?;
    assert!(connection.connection().is_some());
    let outcome = verify_postgres(&mut connection).await;
    connection.execute("ROLLBACK", &mut DiscardSink).await?;
    Box::new(connection).close().await?;
    outcome
}

async fn verify_postgres(connection: &mut PgSession) -> TestResult {
    for sql in [
        "CREATE SCHEMA result_resolution_a",
        "CREATE SCHEMA result_resolution_b",
        "CREATE TABLE result_resolution_a.users (id integer PRIMARY KEY, name text)",
        "CREATE TABLE result_resolution_b.users (id integer PRIMARY KEY, name text)",
        "CREATE VIEW result_resolution_a.user_view AS SELECT * FROM result_resolution_a.users",
        "CREATE TABLE result_resolution_a.\"odd.name\" (id integer PRIMARY KEY)",
        "SET LOCAL search_path = result_resolution_b, result_resolution_a",
    ] {
        connection.execute(sql, &mut DiscardSink).await?;
    }
    assert_eq!(
        resolve(connection, Driver::Postgres, "users").await?,
        SourceResolution::Table {
            schema: "result_resolution_b".into(),
            table: "users".into()
        }
    );
    connection
        .execute(
            "SET LOCAL search_path = result_resolution_a",
            &mut DiscardSink,
        )
        .await?;
    assert_eq!(
        resolve(connection, Driver::Postgres, "users").await?,
        SourceResolution::Table {
            schema: "result_resolution_a".into(),
            table: "users".into()
        }
    );
    assert_eq!(
        resolve(connection, Driver::Postgres, "odd.name").await?,
        SourceResolution::Table {
            schema: "result_resolution_a".into(),
            table: "odd.name".into()
        }
    );
    assert!(reason(resolve(connection, Driver::Postgres, "user_view").await?).contains("view"));
    assert!(
        reason(resolve(connection, Driver::Postgres, "users'; SELECT 1; --").await?)
            .contains("not found")
    );
    connection
        .execute("CREATE TEMP TABLE users (id integer)", &mut DiscardSink)
        .await?;
    assert!(reason(resolve(connection, Driver::Postgres, "users").await?).contains("Temporary"));
    Ok(())
}

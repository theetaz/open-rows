// SPDX-License-Identifier: Apache-2.0
use super::*;
use openrows_core::SessionManager;
use openrows_driver_api::{DiscardSink, Session};
use openrows_results::ResultStore;
use std::sync::Arc;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn row(schema: &str, kind: &str, persistence: &str) -> [String; 4] {
    [
        schema.into(),
        "users".into(),
        kind.into(),
        persistence.into(),
    ]
}
fn reason(result: SourceResolution) -> String {
    match result {
        SourceResolution::Readonly { reason } => reason,
        SourceResolution::Table { .. } => String::new(),
    }
}

#[test]
fn resolves_only_permanent_unambiguous_base_tables() {
    assert_eq!(
        resolution(&[row("tenant", "r", "p")], Driver::Postgres),
        SourceResolution::Table {
            schema: "tenant".into(),
            table: "users".into()
        }
    );
    for (rows, driver, expected) in [
        (
            vec![row("db", "TEMPORARY", "p"), row("db", "BASE TABLE", "p")],
            Driver::Mariadb,
            "Temporary",
        ),
        (vec![], Driver::Postgres, "not found"),
        (vec![row("tenant", "v", "p")], Driver::Postgres, "view"),
        (vec![row("tenant", "m", "p")], Driver::Postgres, "view"),
        (
            vec![row("pg_temp_3", "r", "t")],
            Driver::Postgres,
            "Temporary",
        ),
        (
            vec![row("main", "table", "p"), row("other", "table", "p")],
            Driver::Sqlite,
            "ambiguous",
        ),
        (
            vec![row("attached", "table", "p")],
            Driver::Sqlite,
            "attached",
        ),
        (
            vec![row("main", "virtual", "p")],
            Driver::Sqlite,
            "non-base",
        ),
    ] {
        assert!(reason(resolution(&rows, driver)).contains(expected));
    }
}

#[test]
fn permanent_databases_named_temp_are_not_temporary_tables() {
    for driver in [Driver::Mysql, Driver::Mariadb] {
        assert_eq!(
            resolution(&[row("temp", "BASE TABLE", "p")], driver),
            SourceResolution::Table {
                schema: "temp".into(),
                table: "users".into()
            }
        );
    }
    assert_eq!(
        resolution(&[row("temp", "r", "p")], Driver::Postgres),
        SourceResolution::Table {
            schema: "temp".into(),
            table: "users".into()
        }
    );
}

#[test]
fn mysql_requires_a_verified_non_temporary_definition() {
    assert!(mysql_definition_reason(Some("CREATE TABLE `users` (id int)")).is_none());
    assert!(
        mysql_definition_reason(Some("CREATE TEMPORARY TABLE `users` (id int)"))
            .is_some_and(|reason| reason.contains("Temporary"))
    );
    for definition in [None, Some(""), Some("CREATE VIEW users AS SELECT 1")] {
        assert!(mysql_definition_reason(definition).is_some());
    }
}

#[test]
fn validates_ipc_name_without_treating_quoted_contents_as_sql() -> TestResult {
    for name in ["", "bad\0name", &"x".repeat(1025)] {
        assert!(validate_name(name).is_err());
    }
    validate_name("odd.schema\"'; --")?;
    let json = serde_json::to_value(resolution(&[row("public", "r", "p")], Driver::Postgres))?;
    assert_eq!(json["kind"], "table");
    assert_eq!(json["schema"], "public");
    Ok(())
}

#[tokio::test]
async fn sqlite_resolution_uses_the_owning_session_and_never_stores_results() -> TestResult {
    let store = Arc::new(ResultStore::default());
    let manager = SessionManager::new(store.clone());
    let session = manager
        .open(async {
            let connection =
                openrows_driver_sqlite::open(std::path::Path::new(":memory:"), false, true)?;
            Ok(Box::new(connection) as Box<dyn Session>)
        })
        .await?;
    let port = manager.facet_port(session)?;
    port.call(|connection| Box::pin(verify_sqlite(connection)))
        .await?;
    assert_eq!(store.memory(), 0);
    manager.close(session).await?;
    assert!(
        port.call(|connection| Box::pin(async {
            resolve(connection, Driver::Sqlite, "users").await
        }))
        .await
        .is_err()
    );
    Ok(())
}

async fn verify_sqlite(connection: &mut dyn Connection) -> Result<(), DriverError> {
    connection
        .execute(
            "CREATE TABLE users (id INTEGER PRIMARY KEY)",
            &mut DiscardSink,
        )
        .await?;
    assert_eq!(
        resolve(connection, Driver::Sqlite, "users").await?,
        SourceResolution::Table {
            schema: "main".into(),
            table: "users".into()
        }
    );
    for (sql, name, expected) in [
        ("CREATE TEMP TABLE users (id INTEGER)", "users", "Temporary"),
        ("DROP TABLE temp.users", "users", ""),
        (
            "CREATE TEMP TABLE temporary_only (id INTEGER)",
            "temporary_only",
            "Temporary",
        ),
        (
            "CREATE VIEW user_view AS SELECT * FROM users",
            "user_view",
            "view",
        ),
        ("SELECT 1", "missing", "not found"),
        ("SELECT 1", "users'; DROP TABLE users; --", "not found"),
        ("ATTACH DATABASE ':memory:' AS other", "users", ""),
        (
            "CREATE TABLE other.users (id INTEGER)",
            "users",
            "ambiguous",
        ),
        (
            "CREATE TABLE other.only_attached (id INTEGER)",
            "only_attached",
            "attached",
        ),
    ] {
        connection.execute(sql, &mut DiscardSink).await?;
        let outcome = resolve(connection, Driver::Sqlite, name).await?;
        if expected.is_empty() {
            assert!(matches!(outcome, SourceResolution::Table { .. }));
        } else {
            assert!(reason(outcome).contains(expected));
        }
    }
    Ok(())
}

#[tokio::test]
async fn catalog_failure_is_not_a_table() -> TestResult {
    let mut connection =
        openrows_driver_sqlite::open(std::path::Path::new(":memory:"), false, true)?;
    assert!(
        resolve(&mut connection, Driver::Postgres, "users")
            .await
            .is_err()
    );
    assert!(
        reason(resolve(&mut connection, Driver::Duckdb, "users").await?)
            .contains("cannot authoritatively")
    );
    Box::new(connection).close().await?;
    Ok(())
}

#[path = "live.rs"]
mod live;

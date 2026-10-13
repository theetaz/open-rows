// SPDX-License-Identifier: Apache-2.0
use super::*;
use openrows_driver_api::{ConnectRequest, ConnectTrace, DiscardSink, Endpoint, Session};

#[tokio::test]
#[ignore = "requires a disposable MySQL or MariaDB server via OPENROWS_TEST_MYSQL_PORT"]
async fn mysql_current_database_and_temporary_shadow() -> Result<(), Box<dyn std::error::Error>> {
    let request = ConnectRequest {
        endpoint: Endpoint::Server {
            host: "127.0.0.1".into(),
            port: std::env::var("OPENROWS_TEST_MYSQL_PORT")?.parse()?,
        },
        database: "mysql".into(),
        user: "root".into(),
        password: None,
        application_name: "source-resolution-test".into(),
        ssl_mode: openrows_driver_api::SslMode::Disable,
        tls: openrows_driver_api::TlsFiles::default(),
        via: None,
        timeouts: openrows_driver_api::ConnectTimeouts::default(),
        driver_options: std::collections::BTreeMap::new(),
        startup_commands: vec![],
    };
    let mut connection =
        openrows_driver_mysql::connect(&request, &mut ConnectTrace::default()).await?;
    assert!(connection.connection().is_some());
    let outcome = verify_mysql(&mut connection).await;
    for sql in [
        "DROP DATABASE IF EXISTS source_resolution_a",
        "DROP DATABASE IF EXISTS source_resolution_b",
    ] {
        connection.execute(sql, &mut DiscardSink).await?;
    }
    Box::new(connection).close().await?;
    Ok(outcome?)
}

async fn verify_mysql(connection: &mut dyn Connection) -> Result<(), DriverError> {
    for sql in [
        "CREATE DATABASE source_resolution_a",
        "CREATE DATABASE source_resolution_b",
        "CREATE TABLE source_resolution_a.users (id INT PRIMARY KEY)",
        "CREATE TABLE source_resolution_b.users (id INT PRIMARY KEY)",
        "CREATE VIEW source_resolution_b.user_view AS SELECT * FROM source_resolution_b.users",
    ] {
        connection.execute(sql, &mut DiscardSink).await?;
    }
    for (sql, schema) in [
        ("USE source_resolution_a", "source_resolution_a"),
        ("USE source_resolution_b", "source_resolution_b"),
    ] {
        connection.execute(sql, &mut DiscardSink).await?;
        assert_eq!(
            resolve(connection, Driver::Mysql, "users").await?,
            SourceResolution::Table {
                schema: schema.into(),
                table: "users".into()
            }
        );
    }
    assert!(matches!(
        resolve(connection, Driver::Mysql, "user_view").await?,
        SourceResolution::Readonly { .. }
    ));
    assert!(matches!(
        resolve(connection, Driver::Mysql, "missing").await?,
        SourceResolution::Readonly { .. }
    ));
    connection
        .execute("CREATE TEMPORARY TABLE users (id INT)", &mut DiscardSink)
        .await?;
    let temporary = resolve(connection, Driver::Mysql, "users").await?;
    assert!(
        matches!(temporary, SourceResolution::Readonly { reason } if reason.contains("Temporary"))
    );
    Ok(())
}

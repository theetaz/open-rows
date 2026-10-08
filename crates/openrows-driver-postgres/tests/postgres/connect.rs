// SPDX-License-Identifier: Apache-2.0
//! Connect options, server info, startup commands and connect failures.
use crate::support::{TestResult, config, single_batch, strings};
use openrows_driver_api::DriverError;
use openrows_driver_postgres::{ConnectOptions, Flavour, PgConfig, PgSession, SslMode, TlsStatus};

#[tokio::test]
#[ignore = "needs PostgreSQL: run `just test-pg`"]
async fn connect_reports_server_info_and_runs_startup_commands() -> TestResult {
    let options = ConnectOptions {
        application_name: "openrows-d04".to_owned(),
        startup_commands: vec![
            "SET search_path TO pg_catalog".to_owned(),
            "SET statement_timeout = '5s'; SET lock_timeout = '1s'".to_owned(),
        ],
        ..ConnectOptions::default()
    };
    let mut session = PgSession::connect_with(&config()?, &options).await?;
    let info = session.server_info().clone();
    assert!(!info.server_version.is_empty());
    assert!(info.version.starts_with("PostgreSQL "), "{}", info.version);
    assert_eq!(
        (info.database.as_str(), info.user.as_str()),
        ("postgres", "postgres")
    );
    assert_eq!(
        (info.flavour, info.product()),
        (Flavour::Postgres, "PostgreSQL")
    );
    assert!(
        matches!(info.tls, TlsStatus::Encrypted(_)),
        "{:?}",
        info.tls
    );
    let sql = "SELECT NULL AS v UNION ALL SELECT current_setting('application_name')
        UNION ALL SELECT current_setting('search_path')
        UNION ALL SELECT current_setting('lock_timeout')";
    let batch = single_batch(&mut session, sql).await?;
    assert_eq!(strings(&batch, "v")?, ["openrows-d04", "pg_catalog", "1s"]);
    Ok(())
}

#[tokio::test]
#[ignore = "needs PostgreSQL: run `just test-pg`"]
async fn disable_mode_and_empty_database_connect_in_plaintext() -> TestResult {
    let config = PgConfig {
        database: String::new(),
        ..config()?
    };
    let options = ConnectOptions {
        ssl_mode: SslMode::Disable,
        ..ConnectOptions::default()
    };
    let session = PgSession::connect_with(&config, &options).await?;
    let info = session.server_info();
    assert_eq!(
        (info.tls.clone(), info.database.as_str()),
        (TlsStatus::Disabled, "postgres")
    );
    Ok(())
}

#[tokio::test]
#[ignore = "needs PostgreSQL: run `just test-pg`"]
async fn connect_failures_are_typed() -> TestResult {
    let tls = ConnectOptions {
        ssl_mode: SslMode::VerifyFull,
        ..ConnectOptions::default()
    };
    let error = PgSession::connect_with(&config()?, &tls).await.err();
    assert!(matches!(error, Some(DriverError::Tls(_))), "{error:?}");

    let failing = ConnectOptions {
        startup_commands: vec!["SELECT 1".to_owned(), "SELEC 2".to_owned()],
        ..ConnectOptions::default()
    };
    match PgSession::connect_with(&config()?, &failing).await.err() {
        Some(DriverError::Server { code, message }) => {
            assert_eq!(code, "42601");
            assert!(message.starts_with("startup command 2: "), "{message}");
        }
        other => unreachable!("expected a server error, got {other:?}"),
    }

    let wrong = PgConfig {
        password: Some("not-the-password".to_owned()),
        ..config()?
    };
    let error = PgSession::connect(&wrong).await.err();
    assert!(matches!(error, Some(DriverError::Auth(_))), "{error:?}");

    let closed = std::net::TcpListener::bind("127.0.0.1:0")?
        .local_addr()?
        .port();
    let refused = PgConfig {
        port: closed,
        ..config()?
    };
    let error = PgSession::connect(&refused).await.err();
    assert!(matches!(error, Some(DriverError::Network(_))), "{error:?}");
    Ok(())
}

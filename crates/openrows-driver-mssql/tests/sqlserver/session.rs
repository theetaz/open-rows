// SPDX-License-Identifier: Apache-2.0
//! Connect, TLS modes, routes, batches, messages, row counts, parameters,
//! transactions, cancel and errors.
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use arrow_array::Array;
use arrow_array::cast::AsArray;
use arrow_array::types::Int32Type;
use openrows_driver_api::{
    ConnectStage, ConnectTrace, Connection, DriverError, Endpoint, NoticeLevel, Session, SslMode,
    TlsStatus,
};
use openrows_driver_mssql::connect;
use openrows_net::testing::{TestProxy, TestProxyConfig};
use openrows_net::{ProxyDialer, ProxyKind, SecretString};

use super::support::{Collect, DATABASE, TestResult, code, request, session};

#[tokio::test]
#[ignore = "needs SQL Server; run with `just test-mssql`"]
async fn connects_with_stages_server_info_and_tls() -> TestResult {
    let mut trace = ConnectTrace::default();
    let session = connect(&request(SslMode::Require)?, &mut trace).await?;
    let stages: Vec<ConnectStage> = trace.completed.iter().map(|(stage, _)| *stage).collect();
    assert_eq!(
        stages,
        [
            ConnectStage::Resolve,
            ConnectStage::Connect,
            ConnectStage::Tls,
            ConnectStage::Tls,
            ConnectStage::Authenticate,
            ConnectStage::ServerInfo,
            ConnectStage::StartupCommands
        ]
    );
    let server = session.server();
    assert!(
        matches!(server.tls, TlsStatus::Encrypted(_)),
        "{:?}",
        server.tls
    );
    assert_eq!(
        (server.product.as_str(), server.database.as_str()),
        ("SQL Server", DATABASE)
    );
    assert!(server.version.starts_with("16."), "{}", server.version);
    assert_eq!(server.user, "sa");
    // The container's certificate is self-signed: full verification fails as TLS.
    let verified = connect(&request(SslMode::VerifyFull)?, &mut ConnectTrace::default()).await;
    assert!(
        matches!(verified, Err(DriverError::Tls(_))),
        "{:?}",
        verified.err()
    );
    for mode in [SslMode::Allow, SslMode::Prefer] {
        let other = connect(&request(mode)?, &mut ConnectTrace::default()).await?;
        Box::new(other).close().await?;
    }
    Box::new(session).close().await?;
    Ok(())
}

#[tokio::test]
#[ignore = "needs SQL Server; run with `just test-mssql`"]
async fn wrong_passwords_and_missing_databases_are_auth_errors() -> TestResult {
    let mut wrong = request(SslMode::Require)?;
    wrong.password = Some(SecretString::from("not-the-password"));
    let mut trace = ConnectTrace::default();
    let failed = connect(&wrong, &mut trace).await;
    assert!(
        matches!(failed, Err(DriverError::Auth(_))),
        "{:?}",
        failed.err()
    );
    assert_eq!(trace.failed, Some(ConnectStage::Authenticate));
    let mut missing = request(SslMode::Require)?;
    missing.database = "no_such_database".to_owned();
    let failed = connect(&missing, &mut ConnectTrace::default()).await;
    assert!(
        matches!(&failed, Err(DriverError::Auth(message)) if message.contains("4060")),
        "{:?}",
        failed.err()
    );
    Ok(())
}

#[tokio::test]
#[ignore = "needs SQL Server; run with `just test-mssql`"]
async fn proxied_routes_carry_the_session() -> TestResult {
    let direct = request(SslMode::Require)?;
    let Endpoint::Server { port, .. } = direct.endpoint else {
        return Err("server endpoint expected".into());
    };
    let config = TestProxyConfig {
        kind: ProxyKind::Socks5,
        credentials: None,
    };
    let proxy = TestProxy::start(config).await?;
    let spec = Arc::new(proxy.spec());
    let routed = direct.through(Arc::new(ProxyDialer::new(spec, "127.0.0.1", port)));
    let mut session = connect(&routed, &mut ConnectTrace::default()).await?;
    let mut sink = Collect::default();
    session.execute("SELECT 1 AS one", &mut sink).await?;
    assert_eq!(sink.rows(), 1);
    Box::new(session).close().await?;
    Ok(())
}

#[tokio::test]
#[ignore = "needs SQL Server; run with `just test-mssql`"]
async fn batches_stream_every_result_with_messages_and_counts() -> TestResult {
    let mut session = session().await?;
    let mut sink = Collect::default();
    let sql = "DECLARE @t TABLE (n int); \
               INSERT INTO @t VALUES (1), (2), (3); \
               PRINT 'hello'; \
               SELECT n FROM @t ORDER BY n; \
               SELECT N'x' AS a, 2 AS b WHERE 1 = 0; \
               RAISERROR('careful', 10, 1);";
    let summary = session.execute(sql, &mut sink).await?;
    assert_eq!(sink.schemas.len(), 2);
    assert_eq!(sink.rows(), 3);
    assert_eq!(summary.command_tag, "DECLARE");
    let notices: Vec<_> = sink
        .notices
        .iter()
        .map(|(level, text)| (*level, text.as_str()))
        .collect();
    assert_eq!(
        notices,
        [
            (NoticeLevel::Info, "(3 rows affected)"),
            (NoticeLevel::Info, "hello"),
            (NoticeLevel::Warning, "careful"),
        ]
    );
    let first = sink.batches[0].column(0).as_primitive::<Int32Type>();
    assert_eq!(first.values(), &[1, 2, 3]);
    Box::new(session).close().await?;
    Ok(())
}

#[tokio::test]
#[ignore = "needs SQL Server; run with `just test-mssql`"]
async fn bound_parameters_are_text_the_server_converts() -> TestResult {
    let mut session = session().await?;
    let mut sink = Collect::default();
    let params = [
        Some("41".to_owned()),
        None,
        Some("h\u{e9}llo \u{4e16}".to_owned()),
    ];
    session
        .execute_bound("SELECT CAST(@P1 AS int) + 1, @P2, @P3", &params, &mut sink)
        .await?;
    let batch = &sink.batches[0];
    assert_eq!(batch.column(0).as_primitive::<Int32Type>().value(0), 42);
    assert!(batch.column(1).is_null(0));
    assert_eq!(
        batch.column(2).as_string::<i32>().value(0),
        "h\u{e9}llo \u{4e16}"
    );
    Box::new(session).close().await?;
    Ok(())
}

#[tokio::test]
#[ignore = "needs SQL Server; run with `just test-mssql`"]
async fn transactions_are_tracked_from_the_server() -> TestResult {
    let mut session = session().await?;
    assert!(!session.in_transaction());
    session
        .execute("BEGIN TRANSACTION", &mut Collect::default())
        .await?;
    assert!(session.in_transaction());
    session
        .execute("SAVE TRANSACTION s1; SELECT 1", &mut Collect::default())
        .await?;
    assert!(session.in_transaction());
    session.execute("ROLLBACK", &mut Collect::default()).await?;
    assert!(!session.in_transaction());
    session
        .execute(
            "SET IMPLICIT_TRANSACTIONS ON; SELECT 1 AS a INTO #t",
            &mut Collect::default(),
        )
        .await?;
    assert!(session.in_transaction());
    session
        .execute(
            "COMMIT; SET IMPLICIT_TRANSACTIONS OFF",
            &mut Collect::default(),
        )
        .await?;
    assert!(!session.in_transaction());
    Box::new(session).close().await?;
    Ok(())
}

#[tokio::test]
#[ignore = "needs SQL Server; run with `just test-mssql`"]
async fn attention_cancels_and_the_session_stays_usable() -> TestResult {
    let mut session = session().await?;
    let handle = session.cancel_handle().ok_or("no cancel handle")?;
    // Nothing runs yet: a cancel is a no-op and the next statement is clean.
    handle.cancel().await?;
    let started = Instant::now();
    let mut sink = Collect::default();
    let done = AtomicBool::new(false);
    let (result, cancelled) = tokio::join!(
        async {
            let result = session.execute("WAITFOR DELAY '00:01:00'", &mut sink).await;
            done.store(true, Ordering::Release);
            result
        },
        async {
            // Repeat until the statement ends: no guessing when it started.
            let mut ticks = tokio::time::interval(Duration::from_millis(100));
            while !done.load(Ordering::Acquire) && started.elapsed() < Duration::from_secs(20) {
                ticks.tick().await;
                handle.cancel().await?;
            }
            Ok::<(), DriverError>(())
        }
    );
    cancelled?;
    assert_eq!(code(result), "HY008");
    assert!(started.elapsed() < Duration::from_secs(20));
    let mut sink = Collect::default();
    session.execute("SELECT 7", &mut sink).await?;
    assert_eq!(
        sink.batches[0]
            .column(0)
            .as_primitive::<Int32Type>()
            .value(0),
        7
    );
    Box::new(session).close().await?;
    Ok(())
}

#[tokio::test]
#[ignore = "needs SQL Server; run with `just test-mssql`"]
async fn errors_keep_the_session_and_name_number_and_line() -> TestResult {
    let mut session = session().await?;
    let failed = session
        .execute("SELECT 1\nSELEC 2", &mut Collect::default())
        .await;
    assert!(
        matches!(&failed, Err(DriverError::Server { code, message }) if code == "42000" && message.contains("error 102, line 2")),
        "{:?}",
        failed.err()
    );
    assert_eq!(
        code(
            session
                .execute("SELECT * FROM no_such_table", &mut Collect::default())
                .await
        ),
        "42S02"
    );
    assert_eq!(
        code(session.execute("SELECT 1/0", &mut Collect::default()).await),
        "22012"
    );
    session.execute("SELECT 1", &mut Collect::default()).await?;
    Box::new(session).close().await?;
    Ok(())
}

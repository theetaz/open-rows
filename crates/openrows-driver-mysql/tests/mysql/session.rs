// SPDX-License-Identifier: Apache-2.0
//! Connect, TLS, typed results, parameters, warnings, cancel and errors.
use std::sync::Arc;
use std::time::{Duration, Instant};

use arrow_array::cast::AsArray;
use arrow_array::types::{Decimal128Type, TimestampMicrosecondType};
use arrow_array::{Array, Int64Array};
use arrow_schema::{DataType, TimeUnit};
use openrows_driver_api::{
    ConnectStage, ConnectTrace, Connection, DriverError, Endpoint, NoticeLevel, Session, SslMode,
    TlsStatus,
};
use openrows_driver_mysql::connect;
use openrows_net::testing::{TestProxy, TestProxyConfig};
use openrows_net::{ProxyDialer, ProxyKind, SecretString};

use super::support::{Collect, DATABASE, TestResult, code, mariadb, request, run, session};

#[tokio::test]
#[ignore = "needs MySQL or MariaDB; run with `just test-mysql`"]
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
            ConnectStage::Authenticate,
            ConnectStage::ServerInfo
        ]
    );
    assert!(
        matches!(trace.tls, Some(TlsStatus::Encrypted(_))),
        "{:?}",
        trace.tls
    );
    let server = session.server();
    let product = if mariadb() { "MariaDB" } else { "MySQL" };
    assert_eq!(
        (server.product.as_str(), server.database.as_str()),
        (product, DATABASE)
    );
    assert!(server.user.starts_with("root@"), "{}", server.user);
    let plain = connect(&request(SslMode::Disable)?, &mut ConnectTrace::default()).await?;
    assert_eq!(plain.server().tls, TlsStatus::Disabled);
    Box::new(plain).close().await?;
    Box::new(session).close().await?;
    Ok(())
}

#[tokio::test]
#[ignore = "needs MySQL or MariaDB; run with `just test-mysql`"]
async fn wrong_passwords_and_missing_databases_are_typed() -> TestResult {
    let mut wrong = request(SslMode::Prefer)?;
    wrong.password = Some(SecretString::from("not-the-password"));
    let failed = connect(&wrong, &mut ConnectTrace::default()).await;
    assert!(
        matches!(failed, Err(DriverError::Auth(_))),
        "{:?}",
        failed.err()
    );
    let mut missing = request(SslMode::Prefer)?;
    missing.database = "no_such_database".to_owned();
    let failed = connect(&missing, &mut ConnectTrace::default()).await;
    assert!(
        matches!(&failed, Err(DriverError::Server { message, .. }) if message.contains("Unknown database")),
        "{:?}",
        failed.err()
    );
    Ok(())
}

#[tokio::test]
#[ignore = "needs MySQL or MariaDB; run with `just test-mysql`"]
async fn results_stream_typed_in_both_protocols() -> TestResult {
    let mut session = session().await?;
    run(&mut session, &[
        "DROP TABLE IF EXISTS kinds",
        "CREATE TABLE kinds (id BIGINT PRIMARY KEY, name VARCHAR(20), price DECIMAL(10,2), at DATETIME(3), doc JSON, raw VARBINARY(4))",
        "INSERT INTO kinds VALUES (1, 'é', 12.50, '2024-05-06 07:08:09.123', '{\"a\": 1}', x'00ff'), (2, NULL, NULL, NULL, NULL, NULL)",
    ]).await?;
    for params in [vec![], vec![Some("0".to_owned())]] {
        let sql = if params.is_empty() {
            "SELECT * FROM kinds ORDER BY id"
        } else {
            "SELECT * FROM kinds WHERE id > ? ORDER BY id"
        };
        let mut sink = Collect::default();
        let summary = session.execute_bound(sql, &params, &mut sink).await?;
        assert_eq!(
            (summary.rows, summary.command_tag.as_str()),
            (2, "SELECT 2")
        );
        let schema = sink.schemas[0].clone();
        let types: Vec<DataType> = schema
            .fields()
            .iter()
            .map(|f| f.data_type().clone())
            .collect();
        assert_eq!(
            types,
            [
                DataType::Int64,
                DataType::Utf8,
                DataType::Decimal128(10, 2),
                DataType::Timestamp(TimeUnit::Microsecond, None),
                DataType::Utf8,
                DataType::Binary
            ]
        );
        let batch = &sink.batches[0];
        let price = batch.column(2).as_primitive::<Decimal128Type>().value(0);
        assert_eq!(price, 1_250, "params {params:?}");
        let at = batch
            .column(3)
            .as_primitive::<TimestampMicrosecondType>()
            .value(0);
        assert_eq!(at, 1_714_979_289_123_000, "params {params:?}");
        let doc = batch.column(4).as_string::<i32>().value(0);
        assert!(doc.contains("\"a\""), "{doc}");
        assert!(batch.column(1).is_null(1));
    }
    let mut big = Collect::default();
    session.execute("WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 1000) SELECT i FROM n", &mut big).await?;
    assert_eq!((big.rows(), big.batches[0].num_rows()), (1000, 200));
    let ids = big.batches[0]
        .column(0)
        .as_any()
        .downcast_ref::<Int64Array>()
        .map(|a| a.value(199));
    assert_eq!(ids, Some(200));
    Ok(())
}

#[tokio::test]
#[ignore = "needs MySQL or MariaDB; run with `just test-mysql`"]
async fn procedures_return_several_results_and_warnings_become_notices() -> TestResult {
    let mut session = session().await?;
    run(
        &mut session,
        &[
            "DROP PROCEDURE IF EXISTS two",
            "CREATE PROCEDURE two() BEGIN SELECT 1 AS a; SELECT 'x' AS b, 2 AS c; END",
        ],
    )
    .await?;
    let mut sink = Collect::default();
    session.execute("CALL two()", &mut sink).await?;
    assert_eq!(sink.schemas.len(), 2);
    assert_eq!(sink.schemas[1].fields().len(), 2);
    let mut warned = Collect::default();
    session
        .execute("SELECT CAST('12abc' AS SIGNED)", &mut warned)
        .await?;
    assert!(
        warned
            .notices
            .iter()
            .any(|(level, message)| *level == NoticeLevel::Warning && message.contains("1292")),
        "{:?}",
        warned.notices
    );
    let duplicate = session
        .execute(
            "CREATE TABLE kinds_dup (id INT PRIMARY KEY); INSERT INTO kinds_dup VALUES (1)",
            &mut Collect::default(),
        )
        .await;
    assert_eq!(code(duplicate), "42000", "multi-statement text is refused");
    Ok(())
}

#[tokio::test]
#[ignore = "needs MySQL or MariaDB; run with `just test-mysql`"]
async fn cancel_kills_the_running_query_and_the_session_stays_usable() -> TestResult {
    let mut session = session().await?;
    let cancel = session.cancel_handle().ok_or("no cancel handle")?;
    let canceller = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(300)).await;
        cancel.cancel().await
    });
    let started = Instant::now();
    let result = session
        .execute(
            "SELECT SLEEP(20) FROM (SELECT 1 UNION SELECT 2) t",
            &mut Collect::default(),
        )
        .await;
    canceller.await??;
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "{:?}",
        started.elapsed()
    );
    // SLEEP returns 1 when interrupted; other statements fail with error 1317.
    assert!(result.is_ok() || code(result) == "70100");
    let mut after = Collect::default();
    session.execute("SELECT 1", &mut after).await?;
    assert_eq!(after.rows(), 1);
    Ok(())
}

#[tokio::test]
#[ignore = "needs MySQL or MariaDB; run with `just test-mysql`"]
async fn proxies_carry_tls_sessions() -> TestResult {
    let proxies = [
        (ProxyKind::Socks5, None),
        (
            ProxyKind::HttpConnect,
            Some(("ops".to_owned(), "proxy-pw-synthetic".to_owned())),
        ),
    ];
    for (kind, credentials) in proxies {
        let proxy = TestProxy::start(TestProxyConfig { kind, credentials }).await?;
        let mut routed = request(SslMode::Require)?;
        let Endpoint::Server { host, port } = &routed.endpoint else {
            return Err("the test request names a server".into());
        };
        let destination = format!("{host}:{port}");
        routed.via = Some(Arc::new(ProxyDialer::new(
            Arc::new(proxy.spec()),
            host,
            *port,
        )));
        let mut trace = ConnectTrace::default();
        let session = connect(&routed, &mut trace).await?;
        assert!(
            matches!(trace.tls, Some(TlsStatus::Encrypted(_))),
            "{kind}: {:?}",
            trace.tls
        );
        assert_eq!(proxy.destinations(), [destination]);
        Box::new(session).close().await?;
    }
    Ok(())
}

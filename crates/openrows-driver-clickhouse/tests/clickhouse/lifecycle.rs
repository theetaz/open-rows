// SPDX-License-Identifier: Apache-2.0
use super::support::{Collect, TestResult, config};
use arrow_array::{Array, cast::AsArray};
use openrows_driver_api::{ConnectTrace, DriverError, Endpoint, Session, SslMode};
use openrows_driver_clickhouse::connect;
use openrows_net::{
    ProxyDialer, ProxyKind,
    testing::{TestProxy, TestProxyConfig},
};
use std::sync::Arc;

#[tokio::test]
#[ignore = "needs ClickHouse; run just test-clickhouse"]
async fn http_tls_routes_and_errors() -> TestResult {
    let mut config = config()?;
    let mut session = connect(&config, &mut ConnectTrace::default()).await?;
    session.ping().await?;
    config.password = Some(openrows_net::SecretString::from("pw"));
    assert!(matches!(
        connect(&config, &mut ConnectTrace::default()).await,
        Err(DriverError::Auth(_))
    ));
    config = super::support::config()?;
    config.endpoint = Endpoint::Server {
        host: "localhost".to_owned(),
        port: std::env::var("OPENROWS_TEST_CLICKHOUSE_TLS_PORT")?.parse()?,
    };
    config.ssl_mode = SslMode::VerifyFull;
    config.tls.ca_file = Some(std::env::var("OPENROWS_TEST_CLICKHOUSE_CA")?.into());
    let mut trace = ConnectTrace::default();
    let mut tls = connect(&config, &mut trace).await?;
    tls.ping().await?;
    assert!(matches!(
        trace.tls,
        Some(openrows_driver_api::TlsStatus::Encrypted(_))
    ));
    drop(tls);
    let proxy = TestProxy::start(TestProxyConfig {
        kind: ProxyKind::Socks5,
        credentials: None,
    })
    .await?;
    config.via = Some(Arc::new(ProxyDialer::new(
        Arc::new(proxy.spec()),
        "localhost",
        std::env::var("OPENROWS_TEST_CLICKHOUSE_TLS_PORT")?.parse()?,
    )));
    let mut routed = connect(&config, &mut ConnectTrace::default()).await?;
    routed.ping().await?;
    Ok(())
}

#[tokio::test]
#[ignore = "needs ClickHouse; run just test-clickhouse"]
async fn bound_parameters_preserve_quotes_backslashes_and_null() -> TestResult {
    let mut session = connect(&config()?, &mut ConnectTrace::default()).await?;
    for value in [
        Some("'\\\n\t; DROP TABLE x --"),
        Some("\\N"),
        Some(""),
        None,
    ] {
        let mut sink = Collect::default();
        session
            .execute_bound(
                "SELECT {p1:Nullable(String)} AS value",
                &[value.map(str::to_owned)],
                &mut sink,
            )
            .await?;
        let batch = sink.batches.first().ok_or("no batch")?;
        let column = batch.column(0).as_string::<i32>();
        if let Some(value) = value {
            assert_eq!(column.value(0), value);
        } else {
            assert!(column.is_null(0));
        }
    }
    Ok(())
}

#[tokio::test]
#[ignore = "needs ClickHouse; run just test-clickhouse"]
async fn startup_commands_and_late_stream_errors_leave_session_usable() -> TestResult {
    let mut config = config()?;
    config.startup_commands = vec!["SET max_threads=1".to_owned()];
    let mut session = connect(&config, &mut ConnectTrace::default()).await?;
    let mut sink = Collect::default();
    session
        .execute(
            "SELECT toString(getSetting('max_threads')) AS value",
            &mut sink,
        )
        .await?;
    assert_eq!(sink.batches[0].column(0).as_string::<i32>().value(0), "1");
    let result = session
        .execute(
            "SELECT number, throwIf(number > 10000) FROM numbers(1000000)",
            &mut Collect::default(),
        )
        .await;
    assert!(
        matches!(result, Err(DriverError::Server { .. })),
        "{result:?}"
    );
    session.ping().await?;
    config.startup_commands = vec!["SELEC 1".to_owned()];
    let result = connect(&config, &mut ConnectTrace::default()).await;
    assert!(
        matches!(result, Err(DriverError::Server { message, .. }) if message.contains("startup command 1"))
    );
    Ok(())
}

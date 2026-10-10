// SPDX-License-Identifier: Apache-2.0
//! Authentication, baton transactions, cancellation and reusability.
use super::support::{self, Collect};
use openrows_driver_api::{ConnectTrace, DiscardSink, DriverError, Session};
use openrows_driver_libsql::connect;
use openrows_net::SecretString;
use std::time::Duration;
#[tokio::test]
#[ignore = "needs authenticated sqld; run just test-libsql"]
async fn wrong_token_is_auth_and_never_echoed() -> Result<(), DriverError> {
    let mut request = support::request()?;
    request.password = Some(SecretString::from("test-only"));
    let mut trace = ConnectTrace::default();
    let error = connect(&request, &mut trace)
        .await
        .err()
        .ok_or_else(|| DriverError::Protocol("wrong token connected".into()))?;
    assert!(matches!(error, DriverError::Auth(_)));
    assert!(!format!("{error:?} {trace:?}").contains("test-only"));
    assert_eq!(
        trace.failed,
        Some(openrows_driver_api::ConnectStage::Authenticate)
    );
    Ok(())
}
#[tokio::test]
#[ignore = "needs authenticated sqld; run just test-libsql"]
async fn interactive_transactions_keep_one_baton_and_rollback() -> Result<(), DriverError> {
    let mut session = support::session().await?;
    session
        .execute(
            "CREATE TABLE libsql_transaction (id INTEGER)",
            &mut DiscardSink,
        )
        .await?;
    session.execute("BEGIN IMMEDIATE", &mut DiscardSink).await?;
    assert!(session.in_transaction());
    session
        .execute(
            "INSERT INTO libsql_transaction VALUES (1)",
            &mut DiscardSink,
        )
        .await?;
    session.execute("ROLLBACK", &mut DiscardSink).await?;
    assert!(!session.in_transaction());
    let mut sink = Collect::default();
    session
        .execute("SELECT count(*) FROM libsql_transaction", &mut sink)
        .await?;
    assert_eq!(sink.value(0, 0).as_deref(), Some("0"));
    session.execute("BEGIN", &mut DiscardSink).await?;
    session
        .execute(
            "INSERT INTO libsql_transaction VALUES (2)",
            &mut DiscardSink,
        )
        .await?;
    session.execute("COMMIT", &mut DiscardSink).await?;
    assert!(!session.in_transaction());
    session
        .execute("DROP TABLE libsql_transaction", &mut DiscardSink)
        .await?;
    Box::new(session).close().await
}
#[tokio::test]
#[ignore = "needs authenticated sqld; run just test-libsql"]
async fn cancel_aborts_the_request_and_session_can_open_a_new_stream() -> Result<(), DriverError> {
    let mut session = support::session().await?;
    let cancel = session
        .cancel_handle()
        .ok_or_else(|| DriverError::Protocol("missing cancel".into()))?;
    let mut clock = tokio::time::interval(Duration::from_millis(50));
    clock.tick().await;
    let long = "WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<1000000000) SELECT sum(x) FROM n";
    let mut sink = DiscardSink;
    let run = session.execute(long, &mut sink);
    let cancel = async {
        clock.tick().await;
        cancel.cancel().await
    };
    let (result, cancelled) = tokio::join!(run, cancel);
    cancelled?;
    assert!(matches!(result, Err(DriverError::Server { code, .. }) if code == "57014"));
    session.ping().await?;
    Box::new(session).close().await
}

#[tokio::test]
#[ignore = "needs authenticated sqld; run just test-libsql"]
async fn cancelling_a_transaction_requires_explicit_reconnect() -> Result<(), DriverError> {
    let mut session = support::session().await?;
    session
        .execute("/* test */ BEGIN", &mut DiscardSink)
        .await?;
    let cancel = session
        .cancel_handle()
        .ok_or_else(|| DriverError::Protocol("missing cancel".into()))?;
    let mut clock = tokio::time::interval(Duration::from_millis(50));
    clock.tick().await;
    let long = "WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<1000000000) SELECT sum(x) FROM n";
    let mut sink = DiscardSink;
    let run = session.execute(long, &mut sink);
    let cancel = async {
        clock.tick().await;
        cancel.cancel().await
    };
    let (result, cancelled) = tokio::join!(run, cancel);
    cancelled?;
    assert!(matches!(result, Err(DriverError::Server { code, .. }) if code == "57014"));
    assert!(
        matches!(session.ping().await, Err(DriverError::Protocol(message)) if message.contains("reconnect explicitly"))
    );
    Box::new(session).close().await
}

#[tokio::test]
#[ignore = "needs authenticated sqld; run just test-libsql"]
async fn request_deadline_invalidates_the_stream() -> Result<(), DriverError> {
    let mut request = support::request()?;
    request
        .driver_options
        .insert("libsql_timeout_seconds".into(), "1".into());
    let mut session = connect(&request, &mut ConnectTrace::default()).await?;
    let long = "WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<1000000000) SELECT sum(x) FROM n";
    assert!(
        matches!(session.execute(long, &mut DiscardSink).await, Err(DriverError::Network(error)) if error.to_string().contains("timed out"))
    );
    assert!(matches!(
        session.ping().await,
        Err(DriverError::Protocol(_))
    ));
    Box::new(session).close().await
}

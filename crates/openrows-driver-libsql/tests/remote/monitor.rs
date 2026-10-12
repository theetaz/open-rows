// SPDX-License-Identifier: Apache-2.0
//! Remote monitoring shows own requests and a measured round trip only.
use super::support;
use openrows_driver_api::{Connection, DiscardSink, DriverError, Session};
use openrows_monitor::{SampleState, Source};

#[tokio::test]
#[ignore = "needs authenticated sqld; run just test-libsql"]
async fn own_requests_and_round_trip_without_server_metrics() -> Result<(), DriverError> {
    let mut user = support::session().await?;
    for sql in [
        "CREATE TABLE IF NOT EXISTS libsql_monitor (name TEXT)",
        "INSERT INTO libsql_monitor VALUES ('secret-value')",
        "SELECT * FROM libsql_monitor WHERE name = 'secret-value'",
    ] {
        user.execute(sql, &mut DiscardSink).await?;
    }
    let mut monitor = support::session().await?;
    let reader = monitor
        .monitor()
        .ok_or_else(|| DriverError::Protocol("monitor reader".into()))?;
    let descriptors = reader.discover().await;
    for descriptor in &descriptors {
        match descriptor.source {
            Source::Database | Source::Statements => {
                assert_eq!(descriptor.state, SampleState::Ok);
            }
            _ => assert!(matches!(&descriptor.state, SampleState::Unavailable(reason)
                if reason.contains("server-wide metrics unavailable over Hrana"))),
        }
    }
    let database = reader.read(Source::Database, 2_000).await;
    assert_eq!(database.state, SampleState::Ok);
    let names: Vec<_> = database.columns.iter().map(|c| c.name.as_str()).collect();
    assert!(names.contains(&"round_trip_ms"));
    assert!(
        !names
            .iter()
            .any(|n| n.contains("cpu") || n.contains("memory"))
    );
    let statements = reader.read(Source::Statements, 2_000).await;
    let text: Vec<_> = statements
        .rows
        .iter()
        .filter_map(|row| row.last().cloned().flatten())
        .collect();
    assert!(
        text.iter()
            .any(|q| q.contains("libsql_monitor WHERE name = ?"))
    );
    assert!(
        text.iter()
            .all(|q| !q.contains("secret-value") && q != "SELECT 1")
    );
    Ok(())
}

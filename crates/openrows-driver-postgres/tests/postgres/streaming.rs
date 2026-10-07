// SPDX-License-Identifier: Apache-2.0
//! Batching, first-batch latency, errors and fixture fidelity against a real server.
use crate::support::{Collect, TestResult, connect, run};
use arrow_array::RecordBatch;
use openrows_driver_api::{BatchPolicy, DriverError, Session};
use openrows_driver_postgres::ResultDecoder;
use openrows_driver_postgres::fixture::{self, FixtureType};
use postgres_protocol::message::backend::Message;
use std::{collections::HashMap, time::Duration};

const FIRST_BATCH_BUDGET: Duration = Duration::from_millis(5);

fn rows(sink: &Collect) -> Vec<usize> {
    sink.batches.iter().map(RecordBatch::num_rows).collect()
}

#[tokio::test]
#[ignore = "needs PostgreSQL: run `just test-pg`"]
async fn batches_follow_the_policy_and_the_first_batch_is_fast() -> TestResult {
    let mut session = connect().await?;
    // A heap scan streams rows as produced; a set-returning function in FROM
    // would materialise all rows server-side before sending the first one.
    let fill = "CREATE TEMP TABLE big AS
        SELECT g, g::text AS t, g::float8 AS f FROM generate_series(1, 200000) AS g";
    run(&mut session, fill, Collect::default()).await.0?;
    let sql = "SELECT g, t, f FROM big";
    // Warm the server's plan and page caches so the measurement reflects the driver.
    run(&mut session, sql, Collect::default()).await.0?;
    let (summary, sink) = run(&mut session, sql, Collect::default()).await;
    let summary = summary.expect("streams");
    let sizes = rows(&sink);
    assert_eq!((sizes[0], sizes[1]), (200, 65_536 / 3));
    assert!(sizes.iter().skip(1).all(|&r| r * 3 <= 65_536));
    assert_eq!(
        (summary.rows, sizes.iter().sum::<usize>()),
        (200_000, 200_000)
    );
    assert_eq!(summary.command_tag, "SELECT 200000");
    let decode = summary.first_batch_latency.expect("first batch measured");
    let total = sink.first_batch_after.expect("sink saw a batch");
    report(decode, total);
    assert!(decode <= FIRST_BATCH_BUDGET, "first batch took {decode:?}");
    Ok(())
}

#[expect(
    clippy::print_stderr,
    reason = "latency evidence for the acceptance report"
)]
fn report(decode: Duration, total: Duration) {
    eprintln!("first batch: {decode:?} after first DataRow; {total:?} after execute()");
}

#[tokio::test]
#[ignore = "needs PostgreSQL: run `just test-pg`"]
async fn errors_are_typed_and_the_session_stays_usable() -> TestResult {
    let mut session = connect().await?;
    let cases = [
        ("SELECT 1/0", "22012"),
        ("SELEC 1", "42601"),
        ("SELECT * FROM missing_table", "42P01"),
    ];
    for (sql, expected) in cases {
        match run(&mut session, sql, Collect::default()).await.0 {
            Err(DriverError::Server { code, .. }) => assert_eq!(code, expected, "{sql}"),
            other => unreachable!("{sql}: expected server error, got {other:?}"),
        }
    }
    let unsupported = run(&mut session, "SELECT $1::int", Collect::default())
        .await
        .0;
    assert!(matches!(unsupported, Err(DriverError::Unsupported(_))));
    let (summary, sink) = run(
        &mut session,
        "SELECT 42::int4 AS answer",
        Collect::default(),
    )
    .await;
    assert_eq!((summary.expect("usable").rows, rows(&sink)), (1, vec![1]));
    Ok(())
}

#[tokio::test]
#[ignore = "needs PostgreSQL: run `just test-pg`"]
async fn statements_without_rows_and_stopped_streams() -> TestResult {
    let mut session = connect().await?;
    let (summary, sink) = run(
        &mut session,
        "CREATE TEMP TABLE t (a int)",
        Collect::default(),
    )
    .await;
    assert_eq!(summary.expect("ddl").command_tag, "CREATE TABLE");
    assert!(sink.schema.is_none());
    let (summary, sink) = run(
        &mut session,
        "SELECT 1 AS a WHERE false",
        Collect::default(),
    )
    .await;
    assert_eq!((summary.expect("empty").rows, sink.batches.len()), (0, 0));
    assert_eq!(sink.schema.map(|s| s.fields().len()), Some(1));
    let stop = Collect {
        stop_after_first: true,
        ..Collect::default()
    };
    let sql = "SELECT g FROM generate_series(1, 100000) AS g";
    let (summary, sink) = run(&mut session, sql, stop).await;
    assert_eq!(
        (summary.expect("stopped").batches, rows(&sink)),
        (1, vec![200])
    );
    let (summary, _) = run(&mut session, "SELECT 1", Collect::default()).await;
    assert_eq!(summary.expect("drained").rows, 1);
    Box::new(session).close().await.expect("terminate");
    Ok(())
}

fn decode_fixture(fixture: &fixture::Fixture) -> TestResult<Vec<RecordBatch>> {
    let mut decoder = ResultDecoder::new(&fixture.columns, &HashMap::new(), BatchPolicy::default());
    let mut frames = fixture.frames.clone();
    let mut batches = Vec::new();
    while let Some(Message::DataRow(body)) = Message::parse(&mut frames)? {
        batches.extend(decoder.decode_row(body.buffer())?);
    }
    batches.extend(decoder.finish()?);
    Ok(batches)
}

/// The benchmark input is synthetic; prove the server sends identical values.
#[tokio::test]
#[ignore = "needs PostgreSQL: run `just test-pg`"]
async fn benchmark_fixture_matches_server_output() -> TestResult {
    let mut session = connect().await?;
    let rows = 1_000;
    let sql = fixture::select_sql(&FixtureType::ALL, rows);
    let (summary, sink) = run(&mut session, &sql, Collect::default()).await;
    summary.expect("fixture query runs");
    let local = decode_fixture(&fixture::generate(&FixtureType::ALL, rows))?;
    assert_eq!(sink.batches.len(), local.len());
    for (server, local) in sink.batches.iter().zip(&local) {
        assert_eq!(server, local);
    }
    Ok(())
}

// SPDX-License-Identifier: Apache-2.0
use super::support::{Collect, TestResult, config};
use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use openrows_driver_api::{ConnectTrace, DriverError, ResultSink, Session, SinkFlow};
use openrows_driver_clickhouse::connect;
use std::time::{Duration, Instant};
#[derive(Default)]
struct Measure {
    rows: usize,
    first: Option<Instant>,
    first_rows: usize,
    largest: usize,
}
impl ResultSink for Measure {
    fn begin_result(&mut self, _: SchemaRef) {}
    fn push_batch(&mut self, batch: RecordBatch) -> SinkFlow {
        if self.first.is_none() {
            self.first = Some(Instant::now());
            self.first_rows = batch.num_rows();
        }
        self.rows += batch.num_rows();
        self.largest = self.largest.max(batch.get_array_memory_size());
        SinkFlow::Continue
    }
}
#[tokio::test]
#[ignore = "needs ClickHouse; run just test-clickhouse"]
async fn million_rows_stream_before_completion_and_stop_early() -> TestResult {
    let mut session = connect(&config()?, &mut ConnectTrace::default()).await?;
    let mut sink = Measure::default();
    let started = Instant::now();
    session.execute("SELECT number FROM numbers(1000000) SETTINGS max_execution_speed=100000, timeout_before_checking_execution_speed=0", &mut sink).await?;
    assert_eq!(sink.rows, 1_000_000);
    assert!(sink.first_rows <= 200);
    assert!(sink.largest < 4_194_304);
    let first = sink.first.ok_or("no batch")?;
    assert!(
        first.duration_since(started) < started.elapsed().saturating_sub(Duration::from_secs(1))
    );
    let mut sink = Collect {
        stop: true,
        ..Collect::default()
    };
    session
        .execute("SELECT number FROM numbers(1000000)", &mut sink)
        .await?;
    assert_eq!(sink.batches.len(), 1);
    session.ping().await?;
    Ok(())
}
#[tokio::test]
#[ignore = "needs ClickHouse; run just test-clickhouse"]
async fn count_query_cancels_within_ten_seconds_and_session_recovers() -> TestResult {
    let mut session = connect(&config()?, &mut ConnectTrace::default()).await?;
    let handle = session.cancel_handle().ok_or("no cancel handle")?;
    let started = Instant::now();
    let mut sink = Collect::default();
    let (executed, cancelled) = tokio::join!(
        session.execute("SELECT count() FROM numbers(1000000000000)", &mut sink),
        async {
            let mut ticks = tokio::time::interval(Duration::from_millis(50));
            ticks.tick().await;
            ticks.tick().await;
            handle.cancel().await
        },
    );
    cancelled?;
    assert!(started.elapsed() < Duration::from_secs(10));
    assert!(
        matches!(&executed, Err(DriverError::Server { code, .. }) if code == "57014"),
        "{executed:?}"
    );
    session.ping().await?;
    Ok(())
}

#[tokio::test]
#[ignore = "needs ClickHouse; scripts/bench-clickhouse.py measures peak RSS"]
#[allow(
    clippy::print_stdout,
    reason = "the benchmark's measured results are its output"
)]
async fn million_row_arrow_ingest_benchmark() -> TestResult {
    let mut session = connect(&config()?, &mut ConnectTrace::default()).await?;
    let mut sink = Measure::default();
    let started = Instant::now();
    let summary = session
        .execute(
            "SELECT number, toInt64(number) AS n FROM numbers(1000000)",
            &mut sink,
        )
        .await?;
    let elapsed = started.elapsed().as_secs_f64();
    assert_eq!(sink.rows, 1_000_000);
    let latency = summary
        .first_batch_latency
        .ok_or("missing decode latency")?;
    println!(
        "ClickHouse ArrowStream: rows=1000000 rows/s={:.0} payload_bytes/s={:.0} first_batch_rows={} first_batch_decode_ms={:.3} largest_batch_bytes={} elapsed_s={elapsed:.3}",
        1_000_000.0 / elapsed,
        16_000_000.0 / elapsed,
        sink.first_rows,
        latency.as_secs_f64() * 1000.0,
        sink.largest
    );
    assert!(latency <= Duration::from_millis(5));
    Ok(())
}

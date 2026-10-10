// SPDX-License-Identifier: Apache-2.0
//! Million-row credit flow, native interruption and bounded parent admission.
#[path = "support/duckdb.rs"]
mod fixture;
#[cfg(target_os = "macos")]
#[path = "support/duckdb_image.rs"]
mod image;
use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use fixture::*;
use openrows_driver_api::{BoxFuture, DriverError, ResultSink, Session, SinkAdmission, SinkFlow};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
#[derive(Default)]
struct Pressure {
    rows: AtomicUsize,
    paused: AtomicBool,
    pushed: tokio::sync::Notify,
    resume: tokio::sync::Notify,
}
struct Sink(Arc<Pressure>);
impl ResultSink for Sink {
    fn begin_result(&mut self, _schema: SchemaRef) {}
    fn push_batch(&mut self, batch: RecordBatch) -> SinkFlow {
        self.0.rows.fetch_add(batch.num_rows(), Ordering::SeqCst);
        self.0.paused.store(true, Ordering::SeqCst);
        self.0.pushed.notify_one();
        SinkFlow::Continue
    }
    fn admission(&self) -> SinkAdmission {
        if self.0.paused.load(Ordering::SeqCst) {
            SinkAdmission::Pause
        } else {
            SinkAdmission::Ready
        }
    }
    fn resume(&mut self) -> BoxFuture<'_, SinkAdmission> {
        Box::pin(async move {
            while self.0.paused.load(Ordering::SeqCst) {
                self.0.resume.notified().await;
            }
            SinkAdmission::Ready
        })
    }
}
#[tokio::test]
async fn million_rows_pause_resume_bounded_and_complete() -> Result<(), Box<dyn std::error::Error>>
{
    let mut connection = open(memory("million-test")).await?;
    let pressure = Arc::new(Pressure::default());
    let mut sink = Sink(pressure.clone());
    let bench = std::env::var_os("OPENROWS_BENCH").is_some();
    let pid = connection.process_id();
    let baseline = if bench { rss(pid)? } else { 0 };
    let mut peak = baseline;
    let mut sample_at = 0;
    let first_decode;
    let started = Instant::now();
    let mut execution = connection.execute("SELECT i FROM range(1000000) t(i)", &mut sink);
    loop {
        tokio::select! {
            result = &mut execution => { let summary = result?; assert_eq!(summary.rows, 1_000_000); first_decode = summary.first_batch_latency; break; },
            () = pressure.pushed.notified() => {
                let count = pressure.rows.load(Ordering::SeqCst);
                assert!(count <= 1_000_000);
                if bench && count >= sample_at {
                    peak = peak.max(rss(pid)?);
                    sample_at = count + 100_000;
                }
                if count <= 200 {
                    assert_eq!(count, 200);
                    assert!(tokio::time::timeout(Duration::from_millis(20), &mut execution).await.is_err());
                    assert_eq!(pressure.rows.load(Ordering::SeqCst), count);
                }
                pressure.paused.store(false, Ordering::SeqCst);
                pressure.resume.notify_one();
            }
        }
    }
    drop(execution);
    assert_eq!(pressure.rows.load(Ordering::SeqCst), 1_000_000);
    let metrics = connection.metrics();
    assert_eq!(metrics.outstanding_grants, 1);
    assert!(metrics.encoded_bytes <= 4 * 1024 * 1024 && metrics.decoded_bytes <= 4 * 1024 * 1024);
    if std::env::var_os("OPENROWS_BENCH").is_some() {
        std::io::Write::write_fmt(
            &mut std::io::stdout(),
            format_args!(
                "DuckDB million rows: {:.3} s; first conversion {:?}; native RSS baseline={} peak={} delta={} bytes (host cap 268435456); {metrics:?}\n",
                started.elapsed().as_secs_f64(),
                first_decode,
                baseline,
                peak,
                peak.saturating_sub(baseline)
            ),
        )?;
    }
    Box::new(connection).close().await?;
    Ok(())
}
#[tokio::test]
async fn native_interrupt_under_one_second_and_reuse() -> Result<(), Box<dyn std::error::Error>> {
    let mut connection = open(memory("interrupt-test")).await?;
    let cancel = connection.cancel_handle().ok_or("cancel missing")?;
    let mut sink = openrows_driver_api::DiscardSink;
    let mut execution = connection.execute(
        "SELECT sum(a.i * b.i) FROM range(100000000) a(i), range(100000000) b(i)",
        &mut sink,
    );
    tokio::select! {
        result = &mut execution => { result?; return Err("long query ended".into()); },
        _ = tokio::time::timeout(Duration::from_millis(50), std::future::pending::<()>()) => {}
    }
    let started = Instant::now();
    cancel.cancel().await?;
    let result = tokio::time::timeout(Duration::from_secs(1), execution).await?;
    assert!(matches!(result, Err(DriverError::Server { code, .. }) if code == "57014"));
    if std::env::var_os("OPENROWS_BENCH").is_some() {
        std::io::Write::write_fmt(
            &mut std::io::stdout(),
            format_args!(
                "DuckDB native interrupt/drain: {:.3} ms\n",
                started.elapsed().as_secs_f64() * 1000.0
            ),
        )?;
    }
    connection.ping().await?;
    assert_eq!(connection.execute("SELECT 1", &mut sink).await?.rows, 1);
    Box::new(connection).close().await?;
    Ok(())
}

fn rss(pid: u32) -> Result<usize, Box<dyn std::error::Error>> {
    #[cfg(unix)]
    {
        let output = std::process::Command::new("ps")
            .args(["-o", "rss=", "-p", &pid.to_string()])
            .output()?;
        if !output.status.success() {
            return Err("RSS sampling failed".into());
        }
        Ok(std::str::from_utf8(&output.stdout)?
            .trim()
            .parse::<usize>()?
            * 1024)
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        Ok(0)
    }
}

// SPDX-License-Identifier: Apache-2.0
//! Opt-in RSS/admission measurement with no retained result rows in the stopped sink.
#[path = "adapter_support/mod.rs"]
mod support;
use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use openrows_driver_api::{BoxFuture, ResultSink, Session, SinkAdmission, SinkFlow};
use std::{io::Write, sync::Arc, time::Duration};
#[derive(Default)]
struct Pressure {
    pushed: tokio::sync::Notify,
}
struct Sink {
    pressure: Arc<Pressure>,
    rows: usize,
}
impl ResultSink for Sink {
    fn begin_result(&mut self, _schema: SchemaRef) {}
    fn push_batch(&mut self, batch: RecordBatch) -> SinkFlow {
        self.rows += batch.num_rows();
        self.pressure.pushed.notify_one();
        SinkFlow::Continue
    }
    fn admission(&self) -> SinkAdmission {
        if self.rows == 0 {
            SinkAdmission::Ready
        } else {
            SinkAdmission::Pause
        }
    }
    fn resume(&mut self) -> BoxFuture<'_, SinkAdmission> {
        Box::pin(std::future::pending())
    }
}
struct StoreSink {
    store: openrows_results::ResultStore,
    peak: usize,
    begun: bool,
}
fn admission(value: openrows_results::Admission) -> SinkAdmission {
    match value {
        openrows_results::Admission::Ready => SinkAdmission::Ready,
        openrows_results::Admission::Pause => SinkAdmission::Pause,
        openrows_results::Admission::Stop => SinkAdmission::Stop,
    }
}
impl ResultSink for StoreSink {
    fn begin_result(&mut self, schema: SchemaRef) {
        self.store.begin("r", schema);
        self.begun = true;
    }
    fn push_batch(&mut self, batch: RecordBatch) -> SinkFlow {
        let flow = self.store.append("r", batch);
        self.peak = self.peak.max(self.store.memory());
        if flow == openrows_results::Append::Full {
            SinkFlow::Stop
        } else {
            SinkFlow::Continue
        }
    }
    fn admission(&self) -> SinkAdmission {
        if self.begun {
            admission(self.store.admission("r"))
        } else {
            SinkAdmission::Ready
        }
    }
    fn resume(&mut self) -> BoxFuture<'_, SinkAdmission> {
        Box::pin(async move { admission(self.store.resume("r").await) })
    }
    fn stopped(&mut self) {
        self.store.mark_truncated("r");
    }
}
async fn spill(failed: bool) -> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir_in(std::env::current_dir()?)?;
    let dir = openrows_results::SpillDir::create(root.path())?;
    let path = dir.path().to_path_buf();
    let store = openrows_results::ResultStore::with_spill(
        openrows_results::Limits {
            max_rows: usize::MAX,
            max_bytes: 1024 * 1024,
        },
        dir,
    )?;
    if failed {
        std::fs::remove_dir_all(path)?;
    }
    let mut sink = StoreSink {
        store,
        peak: 0,
        begun: false,
    };
    let mut connection = support::hosted().await?;
    connection
        .execute(&support::sql(1_000_000, "x"), &mut sink)
        .await?;
    let rows = sink.store.rows("r").ok_or("result never began")?;
    if (!failed && rows != (1_000_000, false)) || (failed && (!rows.1 || rows.0 >= 1_000_000)) {
        return Err("spill fixture outcome".into());
    }
    writeln!(
        std::io::stdout(),
        "host_backpressure_memory failed_spill={failed}: rows={:?} store_peak={} store_budget=1048576 plus one admitted batch; {:?}",
        rows,
        sink.peak,
        connection.metrics()
    )?;
    sink.store.close();
    Box::new(connection).close().await?;
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    if !support::enabled() {
        return Ok(());
    }
    writeln!(
        std::io::stdout(),
        "parent Instant minimum observed resolution={:?}; release; one paired pass per fixture",
        support::timer_resolution()
    )?;
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?
        .block_on(run())
}
async fn rss(pid: u32) -> Result<usize, Box<dyn std::error::Error>> {
    let result = tokio::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &pid.to_string()])
        .output()
        .await?;
    Ok(String::from_utf8(result.stdout)?.trim().parse::<usize>()? * 1024)
}
async fn run() -> Result<(), Box<dyn std::error::Error>> {
    // Two default hosts fill the intentional aggregate admission budget.
    let first = support::hosted().await?;
    let second = support::hosted().await?;
    let rejected = support::supervisor()?.spawn().await;
    if !matches!(
        rejected,
        Err(openrows_host_client::HostFailure::MemoryLimit)
    ) {
        return Err("excess host admitted".into());
    }
    Box::new(first).close().await?;
    Box::new(second).close().await?;
    for rows in [100_000, 1_000_000] {
        paused(rows).await?;
    }
    spill(false).await?;
    spill(true).await?;
    oversized().await?;
    Ok(())
}
async fn paused(rows: usize) -> Result<(), Box<dyn std::error::Error>> {
    let mut connection = support::hosted().await?;
    let pid = connection.process_id();
    let baseline = rss(std::process::id()).await? + rss(pid).await?;
    let cancel = connection.cancel_handle().ok_or("no cancel")?;
    let pressure = Arc::new(Pressure::default());
    let mut sink = Sink {
        pressure: pressure.clone(),
        rows: 0,
    };
    let sql = support::sql(rows, "x");
    let mut execution = connection.execute(&sql, &mut sink);
    tokio::select! { () = pressure.pushed.notified() => {}, result = &mut execution => { result?; return Err("early completion".into()); } }
    let mut peak = baseline;
    let mut last = baseline;
    let mut tick = tokio::time::interval(Duration::from_millis(5));
    for _ in 0..32 {
        tokio::select! { _ = tick.tick() => {}, result = &mut execution => { result?; return Err("paused execution ended".into()); } }
        last = rss(std::process::id()).await? + rss(pid).await?;
        peak = peak.max(last);
    }
    let started = std::time::Instant::now();
    cancel.cancel().await?;
    let _ = tokio::time::timeout(Duration::from_secs(2), execution).await?;
    let cancel_ms = started.elapsed().as_secs_f64() * 1000.0;
    writeln!(
        std::io::stdout(),
        "host_backpressure_memory rows={rows}: accepted_rows={} baseline_rss={baseline} peak_rss={peak} final_rss={last} transient_delta={} bytes target<=16777216; cancel_ms={cancel_ms:.3}; {:?}; source SQL producer default batch policy",
        sink.rows,
        peak.saturating_sub(baseline),
        connection.metrics()
    )?;
    Box::new(connection).close().await?;
    Ok(())
}

async fn oversized() -> Result<(), Box<dyn std::error::Error>> {
    let mut connection = support::hosted().await?;
    let pid = connection.process_id();
    let baseline = rss(std::process::id()).await? + rss(pid).await?;
    let mut peak = baseline;
    let mut sink = support::Count::default();
    let mut execution = connection.execute("OPENROWS_TEST_LARGE 8388608", &mut sink);
    let mut tick = tokio::time::interval(Duration::from_millis(5));
    loop {
        tokio::select! {
            result = &mut execution => { result?; break; },
            _ = tick.tick() => { peak = peak.max(rss(std::process::id()).await? + rss(pid).await?); }
        }
    }
    drop(execution);
    peak = peak.max(rss(std::process::id()).await? + rss(pid).await?);
    writeln!(
        std::io::stdout(),
        "exclusive 8 MiB row: {:?}; sampled aggregate RSS baseline={baseline} peak={peak} delta={}; parent-wide exclusive reservation; excess-host rejection=MemoryLimit",
        connection.metrics(),
        peak.saturating_sub(baseline)
    )?;
    Box::new(connection).close().await?;
    Ok(())
}

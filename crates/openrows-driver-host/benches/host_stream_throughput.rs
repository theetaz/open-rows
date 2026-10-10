// SPDX-License-Identifier: Apache-2.0
//! One opt-in paired end-to-end SQL stream, including Arrow transport and sink admission.
#[path = "adapter_support/mod.rs"]
mod support;
use openrows_driver_api::{ConnectTrace, Driver, Session};
use std::{io::Write, time::Instant};
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
async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut hosted = support::hosted().await?;
    let mut native = openrows_driver_sqlite::SqliteDriver
        .connect(&support::request(), &mut ConnectTrace::default())
        .await?;
    let wide = (0..20)
        .map(|c| format!("x as c{c}"))
        .collect::<Vec<_>>()
        .join(",");
    for (shape, columns, projection) in [
        ("short", 1, "x"),
        ("wide", 20, wide.as_str()),
        (
            "mixed",
            5,
            "x, x*1.25, CASE WHEN x%7=0 THEN NULL ELSE 'test-only ම' END, CAST('test-only' AS BLOB), x%2",
        ),
    ] {
        let sql = support::sql(1_000_000, projection);
        let mut sink = support::Count::default();
        let started = Instant::now();
        native.execute(&sql, &mut sink).await?;
        let baseline = started.elapsed();
        let mut sink = support::Count::default();
        let started = Instant::now();
        hosted.execute(&sql, &mut sink).await?;
        let remote = started.elapsed();
        if sink.rows != 1_000_000 {
            return Err("row count".into());
        }
        let cells = 1_000_000.0 * f64::from(columns);
        writeln!(
            std::io::stdout(),
            "host_stream_throughput {shape}: rows=1000000 columns={columns} native={:.0} cells/s hosted={:.0} cells/s ratio={:.3} budget>=0.800 native_ms={:.3} hosted_ms={:.3}",
            cells / baseline.as_secs_f64(),
            cells / remote.as_secs_f64(),
            baseline.as_secs_f64() / remote.as_secs_f64(),
            baseline.as_secs_f64() * 1000.0,
            remote.as_secs_f64() * 1000.0
        )?;
    }
    writeln!(
        std::io::stdout(),
        "credit high-water: {:?}; native/hosted same default BatchPolicy and counting sink; no dictionaries/extensions in SQL fixture",
        hosted.metrics()
    )?;
    native.close().await?;
    Box::new(hosted).close().await?;
    Ok(())
}

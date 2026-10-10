// SPDX-License-Identifier: Apache-2.0
//! Thirty actual launches, parent spawn to mutually authenticated Ready.
mod support;
use std::{io::Write, time::Instant};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    if !support::enabled() {
        return Ok(());
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    let supervisor = support::supervisor()?;
    let mut raw = support::raw("host_startup")?;
    writeln!(raw, "sample,spawn_to_ready_ns")?;
    let mut samples = Vec::new();
    for index in 0..30 {
        let start = Instant::now();
        let host = runtime.block_on(supervisor.spawn())?;
        let elapsed = start.elapsed();
        writeln!(raw, "{index},{}", elapsed.as_nanos())?;
        samples.push(elapsed);
        runtime.block_on(host.close())?;
    }
    support::report("host_startup all fresh (cold p95 budget 150 ms)", &samples)?;
    support::report("host_startup warm (p95 budget 75 ms)", &samples[1..])?;
    writeln!(
        std::io::stdout(),
        "first={:.3} ms; timer resolution {:?}",
        samples[0].as_secs_f64() * 1000.0,
        support::resolution()
    )?;
    Ok(())
}

// SPDX-License-Identifier: Apache-2.0
//! Parent clocks and retained raw samples; no timing gates in ordinary tests.
use openrows_host_client::{CircuitBreaker, HostSupervisor, LaunchOptions, MemoryBudget};
use std::{io::Write, path::PathBuf, time::Duration};

pub fn options() -> Result<LaunchOptions, Box<dyn std::error::Error>> {
    let executable = PathBuf::from(env!("CARGO_BIN_EXE_host-test-fixture")).canonicalize()?;
    let endpoint_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    Ok(LaunchOptions {
        driver: openrows_host_client::HostDriverId::SqliteTest,
        executable,
        endpoint_root,
        memory: MemoryBudget::default(),
        circuit: CircuitBreaker::default(),
    })
}
pub fn supervisor() -> Result<HostSupervisor, Box<dyn std::error::Error>> {
    Ok(HostSupervisor::new(options()?))
}
pub fn enabled() -> bool {
    std::env::var("OPENROWS_BENCH").as_deref() == Ok("1")
}
pub fn report(name: &str, samples: &[Duration]) -> Result<(), Box<dyn std::error::Error>> {
    let mut sorted = samples.to_vec();
    sorted.sort();
    let p50 = sorted[(samples.len() - 1) / 2];
    let p95 = sorted[(samples.len() * 95).div_ceil(100) - 1];
    let max = sorted[samples.len() - 1];
    writeln!(
        std::io::stdout(),
        "{name}: n={} p50={:.3} ms p95={:.3} ms max={:.3} ms",
        samples.len(),
        p50.as_secs_f64() * 1000.0,
        p95.as_secs_f64() * 1000.0,
        max.as_secs_f64() * 1000.0
    )?;
    Ok(())
}
pub fn raw(name: &str) -> Result<std::fs::File, Box<dyn std::error::Error>> {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../perf-results");
    std::fs::create_dir_all(&directory)?;
    let mut file =
        std::fs::File::create(directory.join(format!("{name}-{}.csv", std::process::id())))?;
    writeln!(
        file,
        "# parent Instant; platform={}; arch={}; release profile; samples include first launch",
        std::env::consts::OS,
        std::env::consts::ARCH
    )?;
    Ok(file)
}
pub fn resolution() -> Duration {
    let mut previous = std::time::Instant::now();
    let mut minimum = Duration::MAX;
    for _ in 0..1000 {
        let start = std::time::Instant::now();
        let delta = start.duration_since(previous);
        previous = start;
        if !delta.is_zero() {
            minimum = minimum.min(delta);
        }
    }
    minimum
}

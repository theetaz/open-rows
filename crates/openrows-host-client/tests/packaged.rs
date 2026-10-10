// SPDX-License-Identifier: Apache-2.0
//! Opt-in installed artifact smoke and first/warm handshake measurements, without a GUI.
use openrows_host_client::{DriverHostRuntime, HANDSHAKE_TIMEOUT, HostDriverId, HostState};
use std::{path::PathBuf, time::Instant};

#[tokio::test]
async fn packaged_helper_handshake_and_clean_shutdown() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var("OPENROWS_PACKAGED_SMOKE").as_deref() != Ok("1") {
        return Ok(());
    }
    let app = if let Some(bundle) = std::env::var_os("OPENROWS_PACKAGED_APP") {
        PathBuf::from(bundle).join("Contents/MacOS/openrows-app")
    } else {
        PathBuf::from(
            std::env::var_os("OPENROWS_PACKAGED_EXECUTABLE")
                .ok_or("provide an absolute packaged app executable")?,
        )
    };
    assert!(app.is_file(), "packaged main executable missing");
    let directory = tempfile::Builder::new()
        .prefix(".p-")
        .tempdir_in(short_root())?;
    let runtime = DriverHostRuntime::new(app, directory.path().to_path_buf());
    let mut times = Vec::new();
    for sample in 0..30 {
        let start = Instant::now();
        let launcher = runtime.launcher(HostDriverId::Probe, "test-only")?;
        let session = launcher.spawn().await?;
        let elapsed = start.elapsed();
        assert!(elapsed < HANDSHAKE_TIMEOUT);
        let endpoints = session.directory().to_path_buf();
        let state = session.subscribe();
        session.close().await?;
        assert_eq!(*state.borrow(), HostState::Closed);
        assert!(!endpoints.exists());
        times.push(elapsed.as_secs_f64() * 1000.0);
        print_sample(sample, times[sample]);
    }
    let first = times.remove(0);
    times.sort_by(f64::total_cmp);
    print_summary(first, &times);
    Ok(())
}
fn short_root() -> PathBuf {
    if cfg!(unix) {
        "/tmp".into()
    } else {
        std::env::temp_dir()
    }
}

#[expect(
    clippy::print_stdout,
    reason = "opt-in benchmark evidence, not application diagnostics"
)]
fn print_sample(sample: usize, elapsed: f64) {
    println!("packaged sample {sample}: {elapsed:.3} ms; closed, endpoints removed");
}
#[expect(
    clippy::print_stdout,
    reason = "opt-in benchmark evidence, not application diagnostics"
)]
fn print_summary(first: f64, times: &[f64]) {
    println!(
        "packaged first={first:.3} ms; warm p50={:.3} p95={:.3} max={:.3} ms; deadline=2000 ms",
        times[14], times[27], times[28]
    );
}

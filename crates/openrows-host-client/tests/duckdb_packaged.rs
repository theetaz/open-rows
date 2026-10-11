// SPDX-License-Identifier: Apache-2.0
//! Opt-in packaged `DuckDB` handshake measurements use the installed production helper.
use openrows_host_client::{DriverHostRuntime, HostDriverId, HostState};
use std::{io::Write, path::PathBuf, time::Instant};
#[tokio::test]
async fn packaged_duckdb_first_and_warm_handshake() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var("OPENROWS_BENCH").as_deref() != Ok("1") {
        return Ok(());
    }
    let Some(app) = std::env::var_os("OPENROWS_PACKAGED_APP") else {
        return Ok(());
    };
    let directory = tempfile::Builder::new()
        .prefix(".d-")
        .tempdir_in(if cfg!(unix) {
            PathBuf::from("/tmp")
        } else {
            std::env::temp_dir()
        })?;
    let runtime = DriverHostRuntime::new(
        PathBuf::from(app).join("Contents/MacOS/openrows-app"),
        directory.path().to_path_buf(),
    );
    let mut times = Vec::new();
    for _ in 0..30 {
        let launch = runtime.launcher(HostDriverId::Duckdb, "test-only")?;
        let started = Instant::now();
        let host = launch.spawn().await?;
        times.push(started.elapsed().as_secs_f64() * 1000.0);
        let state = host.subscribe();
        let directory = host.directory().to_owned();
        host.close().await?;
        assert_eq!(*state.borrow(), HostState::Closed);
        assert!(!directory.exists());
    }
    writeln!(
        std::io::stdout(),
        "DuckDB linked packaged raw handshake ms: {times:?}"
    )?;
    let first = times.remove(0);
    times.sort_by(f64::total_cmp);
    writeln!(
        std::io::stdout(),
        "DuckDB linked packaged handshake: first={first:.3} ms; warm n=29 p50={:.3} p95={:.3} max={:.3} ms; deadline=2000 ms",
        times[14],
        times[27],
        times[28]
    )?;
    Ok(())
}

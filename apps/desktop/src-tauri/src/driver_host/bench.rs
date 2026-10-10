// SPDX-License-Identifier: Apache-2.0
//! Opt-in packaged app measurement, through the production runtime and adapter.
use openrows_driver_api::{
    ConnectRequest, ConnectTrace, DiscardSink, Endpoint, FileSource, Session,
};
use openrows_host_client::{
    DriverHostRuntime, DuckdbMetadata, HostDriverId, HostedDriver, PrewarmReport,
};
use std::{path::Path, sync::Arc, time::Instant};

pub(super) async fn measure(runtime: &DriverHostRuntime, prewarm: PrewarmReport) {
    let Some(path) = std::env::var_os("OPENROWS_HOST_BENCH_REPORT") else {
        return;
    };
    let started = Instant::now();
    let measured = connect(runtime).await;
    let (handshake_ms, connect_ms, error) = match measured {
        Ok((handshake, connect)) => (Some(handshake), Some(connect), None),
        Err(error) => (None, None, Some(error)),
    };
    let report = serde_json::json!({
        "pid": std::process::id(),
        "prewarm_outcome": format!("{:?}", prewarm.outcome),
        "prewarm_ms": prewarm.elapsed.as_secs_f64() * 1000.0,
        "prewarm_handshake_ms": prewarm.handshake.map(|time| time.as_secs_f64() * 1000.0),
        "first_connect_handshake_ms": handshake_ms,
        "first_connect_ms": connect_ms,
        "connect_query_close_ms": started.elapsed().as_secs_f64() * 1000.0,
        "error": error,
    });
    if let Err(error) = write(Path::new(&path), &report) {
        tracing::warn!(kind = ?error.kind(), "driver host benchmark report unavailable");
    }
}

async fn connect(runtime: &DriverHostRuntime) -> Result<(f64, f64), &'static str> {
    let launcher = runtime
        .launcher(HostDriverId::Duckdb, "packaged-benchmark")
        .map_err(|_| "helper resolution failed")?;
    let driver = HostedDriver::new(launcher, Arc::new(DuckdbMetadata));
    let started = Instant::now();
    let mut connection = driver
        .open(&request(), &mut ConnectTrace::default())
        .await
        .map_err(|_| "DuckDB connect failed")?;
    let connect_ms = started.elapsed().as_secs_f64() * 1000.0;
    let handshake_ms = connection.handshake_duration().as_secs_f64() * 1000.0;
    let result = connection.execute("SELECT 1", &mut DiscardSink).await;
    let closed = Box::new(connection).close().await;
    let summary = result.map_err(|_| "DuckDB query failed")?;
    if summary.rows != 1 {
        return Err("DuckDB query returned unexpected rows");
    }
    closed.map_err(|_| "DuckDB close failed")?;
    Ok((handshake_ms, connect_ms))
}

fn request() -> ConnectRequest {
    ConnectRequest {
        endpoint: Endpoint::File {
            source: FileSource::Memory {
                name: "packaged-benchmark".into(),
            },
            attached: vec![],
        },
        database: "main".into(),
        user: String::new(),
        password: None,
        application_name: "openrows-benchmark".into(),
        ssl_mode: openrows_driver_api::SslMode::Disable,
        tls: openrows_driver_api::TlsFiles::default(),
        via: None,
        timeouts: openrows_driver_api::ConnectTimeouts::default(),
        driver_options: std::collections::BTreeMap::default(),
        startup_commands: vec![],
    }
}

fn write(path: &Path, report: &serde_json::Value) -> std::io::Result<()> {
    let partial = path.with_extension("partial");
    std::fs::write(&partial, serde_json::to_vec(report)?)?;
    std::fs::rename(partial, path)
}

// SPDX-License-Identifier: Apache-2.0
//! Paired identical in-process/host producers, timed by one parent monotonic clock.
mod support;
use openrows_host_client::{HostFailure, HostSession};
use openrows_host_protocol::{
    ArrowDecoder, Control, OwnedConnect, Reply, WireEndpoint, WireFile, WireTls,
};
use std::{
    io::Write,
    time::{Duration, Instant},
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    if !support::enabled() {
        return Ok(());
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    runtime.block_on(run())
}
async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut host = support::supervisor()?.spawn().await?;
    host.send(1, &Control::Connect(config()))?;
    terminal(&mut host).await?;
    let mut raw = support::raw("host_batch_transport")?;
    writeln!(
        raw,
        "rows,columns,sample,in_process_ns,host_ns,incremental_ns,encoded_bytes"
    )?;
    let mut request = 2;
    for (rows, columns) in [(200, 20), (3277, 20)] {
        let mut overhead = Vec::new();
        let mut baseline = Vec::new();
        let mut hosted = Vec::new();
        for sample in 0..samples() {
            let start = Instant::now();
            let batch = openrows_driver_host::fixture(rows, columns)?;
            std::hint::black_box(&batch);
            let local = start.elapsed();
            drop(batch);
            let (remote, bytes) = transport(
                &mut host,
                request,
                &format!("OPENROWS_TEST_BATCH {rows} {columns}"),
            )
            .await?;
            request += 1;
            let incremental = remote.saturating_sub(local);
            baseline.push(local);
            hosted.push(remote);
            overhead.push(incremental);
            writeln!(
                raw,
                "{rows},{columns},{sample},{},{},{},{bytes}",
                local.as_nanos(),
                remote.as_nanos(),
                incremental.as_nanos()
            )?;
        }
        support::report(&format!("{rows}x{columns} in-process"), &baseline)?;
        support::report(&format!("{rows}x{columns} host"), &hosted)?;
        support::report(
            &format!("{rows}x{columns} incremental (p95 budget 1 ms)"),
            &overhead,
        )?;
    }
    large(&mut host, request).await?;
    writeln!(
        std::io::stdout(),
        "timer resolution {:?}; dictionaries/extensions covered by fidelity tests; fixture mixed int/decimal/text/binary/null",
        support::resolution()
    )?;
    host.close().await?;
    Ok(())
}
fn config() -> OwnedConnect {
    OwnedConnect {
        endpoint: WireEndpoint::File {
            source: WireFile::Memory {
                name: "test-only".into(),
            },
            attached: vec![],
        },
        database: "main".into(),
        user: String::new(),
        password: None,
        application: "bench".into(),
        ssl_mode: 0,
        tls: WireTls::default(),
        timeouts_ms: [5000, 10000, 10000, 15000],
        startup: vec![],
        options: vec![],
    }
}
async fn terminal(host: &mut HostSession) -> Result<(), HostFailure> {
    loop {
        if matches!(
            host.recv()
                .await?
                .decode_control()
                .map_err(HostFailure::from)?,
            Control::Reply(_) | Control::Error(_)
        ) {
            return Ok(());
        }
    }
}
async fn transport(
    host: &mut HostSession,
    request: u64,
    sql: &str,
) -> Result<(Duration, usize), Box<dyn std::error::Error>> {
    let start = Instant::now();
    host.send(request, &Control::Execute { sql: sql.into() })?;
    let mut decoder = ArrowDecoder::new();
    let mut delivered = None;
    let mut encoded_size = 0;
    loop {
        let dto = host.recv().await?.decode_control()?;
        match dto {
            Control::BatchReady(grant) => {
                host.send(0, &Control::Grant(grant))?;
                let bytes = data(host, grant.encoded as usize).await?;
                let batch = decoder.admit(&bytes, grant)?.ok_or("missing batch")?;
                std::hint::black_box(&batch);
                delivered = Some(start.elapsed());
                encoded_size = bytes.len();
                host.send(0, &Control::Consumed(grant.key))?;
            }
            Control::ResultEnd { .. } => decoder.finish(&data(host, 8).await?)?,
            Control::Reply(Reply::Executed(_)) => {
                return Ok((delivered.ok_or("missing sink delivery")?, encoded_size));
            }
            Control::Error(_) => return Err("host operation failed".into()),
            _ => {}
        }
    }
}
async fn data(host: &mut HostSession, size: usize) -> Result<Vec<u8>, HostFailure> {
    let mut bytes = Vec::with_capacity(size);
    while bytes.len() < size {
        bytes.extend_from_slice(&host.read_data().await?.body);
    }
    Ok(bytes)
}

async fn large(host: &mut HostSession, mut request: u64) -> Result<(), Box<dyn std::error::Error>> {
    let size = 63 * 1024 * 1024;
    let mut raw = support::raw("host_large_transport")?;
    writeln!(
        raw,
        "sample,in_process_ns,host_ns,encoded_bytes,mib_per_second"
    )?;
    let mut samples = Vec::new();
    let mut total = Duration::ZERO;
    let mut total_bytes = 0;
    for sample in 0..5 {
        let start = Instant::now();
        let batch = openrows_driver_host::fixture_large(size)?;
        std::hint::black_box(&batch);
        let baseline = start.elapsed();
        drop(batch);
        let (duration, bytes) =
            transport(host, request, &format!("OPENROWS_TEST_LARGE {size}")).await?;
        request += 1;
        total += duration;
        total_bytes += bytes;
        samples.push(duration);
        let throughput =
            f64::from(u32::try_from(bytes)?) / (1024.0 * 1024.0) / duration.as_secs_f64();
        writeln!(
            raw,
            "{sample},{},{},{bytes},{throughput:.3}",
            baseline.as_nanos(),
            duration.as_nanos()
        )?;
    }
    support::report(
        "exclusive row (63 MiB payload within 64 MiB ceiling)",
        &samples,
    )?;
    writeln!(
        std::io::stdout(),
        "exclusive throughput {:.3} MiB/s (target >= 200 MiB/s)",
        f64::from(u32::try_from(total_bytes)?) / (1024.0 * 1024.0) / total.as_secs_f64()
    )?;
    Ok(())
}

fn samples() -> usize {
    std::env::var("OPENROWS_BENCH_SAMPLES")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(30)
        .clamp(30, 5000)
}

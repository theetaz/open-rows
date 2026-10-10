// SPDX-License-Identifier: Apache-2.0
//! Prepare the debug image before testing the fixed two-second protocol deadline.
use openrows_host_client::{HostFailure, MemoryBudget, launch_command};
use std::{io::Write, process::Stdio, time::Duration};
use tokio::{sync::OnceCell, time::Instant};

/// Run one bounded closed-stdin image probe per test process.
pub(super) async fn prepare() -> Result<(), HostFailure> {
    static IMAGE: OnceCell<()> = OnceCell::const_new();
    IMAGE.get_or_try_init(probe).await?;
    Ok(())
}
#[allow(
    clippy::explicit_write,
    reason = "designated acceptance startup measurement matches the existing timing harness"
)]
async fn probe() -> Result<(), HostFailure> {
    // Concurrent macOS debug startup can consume the deadline before any IPC.
    // No bootstrap means no endpoint, credentials or database; real handshakes are never retried.
    let mut child = launch_command(&super::options(MemoryBudget::default()))?
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| HostFailure::Spawn)?;
    let started = Instant::now();
    let status =
        if let Ok(status) = tokio::time::timeout(Duration::from_secs(10), child.wait()).await {
            status.map_err(|_| HostFailure::Spawn)?
        } else {
            let _ = child.kill().await;
            let _ = child.wait().await;
            return Err(HostFailure::Unresponsive);
        };
    if status.code() != Some(70) {
        return Err(HostFailure::Spawn);
    }
    let _ = writeln!(
        std::io::stderr(),
        "Debug image preflight: {:.3} ms",
        started.elapsed().as_secs_f64() * 1000.0
    );
    Ok(())
}

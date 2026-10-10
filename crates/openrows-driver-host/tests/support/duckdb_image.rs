// SPDX-License-Identifier: Apache-2.0
//! Prepare the larger native debug image before the fixed two-second handshake deadline.
use openrows_host_client::{HostFailure, MemoryBudget, launch_command};
use std::{
    process::Stdio,
    time::{Duration, Instant},
};
use tokio::sync::OnceCell;

pub(super) async fn prepare() -> Result<(), HostFailure> {
    static IMAGE: OnceCell<()> = OnceCell::const_new();
    IMAGE.get_or_try_init(probe).await?;
    Ok(())
}
async fn probe() -> Result<(), HostFailure> {
    let mut command = launch_command(&super::options(MemoryBudget::default()))?.into_std();
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| HostFailure::Spawn)?;
    let started = Instant::now();
    let mut ticks = tokio::time::interval(Duration::from_millis(10));
    loop {
        if let Some(status) = child.try_wait().map_err(|_| HostFailure::Spawn)? {
            return if status.code() == Some(70) {
                Ok(())
            } else {
                Err(HostFailure::Spawn)
            };
        }
        if started.elapsed() >= Duration::from_secs(30) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(HostFailure::Unresponsive);
        }
        ticks.tick().await;
    }
}

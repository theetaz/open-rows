// SPDX-License-Identifier: Apache-2.0
//! Disposable parent used to prove child lifetime and endpoint cleanup after SIGKILL.
use openrows_host_client::{CircuitBreaker, HostSupervisor, LaunchOptions, MemoryBudget};
use std::{
    io::{Read, Write},
    path::PathBuf,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let executable = PathBuf::from(std::env::args().nth(1).ok_or("missing helper")?);
    let endpoint_root = PathBuf::from(std::env::args().nth(2).ok_or("missing root")?);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    let options = LaunchOptions {
        driver: openrows_host_client::HostDriverId::SqliteTest,
        executable,
        endpoint_root,
        memory: MemoryBudget::default(),
        circuit: CircuitBreaker::default(),
    };
    let session = runtime.block_on(HostSupervisor::new(options).spawn())?;
    writeln!(
        std::io::stdout(),
        "{} {}",
        session.pid(),
        session.directory().display()
    )?;
    std::io::stdout().flush()?;
    let mut byte = [0];
    let _ = std::io::stdin().read(&mut byte)?;
    runtime.block_on(session.close())?;
    Ok(())
}

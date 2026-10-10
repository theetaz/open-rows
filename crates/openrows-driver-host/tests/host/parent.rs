// SPDX-License-Identifier: Apache-2.0
//! Killing the parent leaves no helper process or private endpoints behind.
use crate::support;
use std::{process::Stdio, time::Duration};
use tokio::io::{AsyncBufReadExt, BufReader};
#[tokio::test]
async fn parent_death_exits_child_and_cleans_endpoints() -> Result<(), Box<dyn std::error::Error>> {
    support::prepare_image().await?;
    let mut parent = tokio::process::Command::new(env!("CARGO_BIN_EXE_host-parent-fixture"))
        .arg(env!("CARGO_BIN_EXE_openrows-driver-host"))
        .arg(support::root())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()?;
    let mut output = BufReader::new(parent.stdout.take().ok_or("missing output")?);
    let mut line = String::new();
    tokio::time::timeout(Duration::from_secs(4), output.read_line(&mut line)).await??;
    let (pid, path) = line.trim_end().split_once(' ').ok_or("missing identity")?;
    let pid = pid.parse::<u32>()?;
    let directory = std::path::PathBuf::from(path);
    parent.kill().await?;
    parent.wait().await?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(4);
    while directory.exists() || process_exists(pid) {
        assert!(
            tokio::time::Instant::now() < deadline,
            "orphan helper {pid}"
        );
        tokio::task::yield_now().await;
    }
    support::assert_reaped(pid, &directory);
    Ok(())
}
fn process_exists(pid: u32) -> bool {
    #[cfg(unix)]
    {
        rustix::process::Pid::from_raw(i32::try_from(pid).unwrap())
            .is_some_and(|pid| rustix::process::test_kill_process(pid).is_ok())
    }
    #[cfg(windows)]
    {
        openrows_host_client::is_process_alive(pid).unwrap()
    }
}

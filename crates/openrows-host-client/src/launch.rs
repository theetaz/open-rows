// SPDX-License-Identifier: Apache-2.0
//! Fixed-path launch with a cleared environment and secret-free command surface.
use crate::{CircuitBreaker, HostFailure, MemoryBudget};
use std::{path::PathBuf, process::Stdio};
use tokio::process::Command;

/// Development/headless callers inject the already-built absolute helper path.
#[derive(Clone)]
pub struct LaunchOptions {
    /// Resolved helper; installed resolution/signature verification belongs to M2-01.4.
    pub executable: PathBuf,
    /// Directory under which private rendezvous directories are created.
    pub endpoint_root: PathBuf,
    /// Shared parent-wide reservation policy.
    pub memory: MemoryBudget,
    /// This profile/driver's abnormal-exit policy.
    pub circuit: CircuitBreaker,
}
/// Inspectable command contains no endpoints, SQL, passwords or bootstrap token.
/// # Errors
/// Helper path is not absolute or does not identify a file.
pub fn launch_command(options: &LaunchOptions) -> Result<Command, HostFailure> {
    if !options.executable.is_absolute() || !options.executable.is_file() {
        return Err(HostFailure::Spawn);
    }
    let mut command = Command::new(&options.executable);
    command.arg("sqlite").env_clear().env("LANG", "C.UTF-8");
    #[cfg(windows)]
    command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW, independent of inherited pipe handles.
    #[cfg(windows)]
    if let Some(root) = std::env::var_os("SystemRoot") {
        command.env("SystemRoot", root);
    }
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    Ok(command)
}

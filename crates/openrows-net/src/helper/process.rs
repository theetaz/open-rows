// SPDX-License-Identifier: Apache-2.0
//! Process groups and cancellable TCP readiness; dropping always kills the tree.
use super::{HelperError, HelperHop, stderr::Stderr};
use std::{
    net::{SocketAddr, TcpListener},
    path::PathBuf,
    process::Stdio,
    time::Duration,
};
use tokio::{
    net::TcpStream,
    process::{Child, Command},
    task::JoinHandle,
};

/// A resolved tool and the version shown in diagnostics.
#[derive(Clone, Debug)]
pub struct HelperSpec {
    /// Structured route.
    pub hop: HelperHop,
    /// Discovered executable or Settings override.
    pub path: PathBuf,
    /// Detected version.
    pub version: String,
    /// TCP readiness deadline (30 seconds by default).
    pub timeout: Duration,
    /// Extra environment for isolated test injection (no credentials).
    pub environment: Vec<(String, String)>,
}
impl HelperSpec {
    /// Resolved tool with the default readiness deadline.
    #[must_use]
    pub fn new(hop: HelperHop, path: PathBuf, version: String) -> Self {
        Self {
            hop,
            path,
            version,
            timeout: Duration::from_secs(30),
            environment: Vec::new(),
        }
    }
    pub(super) fn failure(&self, reason: impl Into<String>, stderr: &Stderr) -> HelperError {
        HelperError::Network {
            tool: self.hop.tool().into(),
            message: reason.into(),
            stderr: stderr.text(),
            hint: self.hop.hint().into(),
        }
    }
    /// Unambiguous argument display, with no shell interpretation.
    ///
    /// # Errors
    /// Invalid hop fields.
    pub fn command_line(&self, port: u16) -> Result<String, HelperError> {
        let mut args = vec![self.path.display().to_string()];
        args.extend(self.hop.arguments(port)?);
        Ok(args
            .iter()
            .map(|value| format!("{value:?}"))
            .collect::<Vec<_>>()
            .join(" "))
    }
}

pub(super) struct Process {
    pub(super) child: Child,
    pub(super) pid: u32,
    pub(super) reader: Option<JoinHandle<()>>,
}
impl Process {
    pub(super) fn spawn(
        spec: &HelperSpec,
        port: u16,
        stderr: &Stderr,
    ) -> Result<Self, HelperError> {
        let mut command = Command::new(&spec.path);
        command
            .args(spec.hop.arguments(port)?)
            .envs(spec.environment.iter().cloned());
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt as _;
            command.as_std_mut().process_group(0);
        }
        let mut child = command.spawn().map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                HelperError::NotFound {
                    tool: spec.hop.tool().into(),
                }
            } else {
                spec.failure(format!("Could not start: {error}"), stderr)
            }
        })?;
        let pid = child
            .id()
            .ok_or_else(|| spec.failure("Missing helper PID", stderr))?;
        let reader = child.stderr.take().map(|pipe| {
            let stderr = stderr.clone();
            tokio::spawn(async move {
                stderr.capture(pipe).await;
            })
        });
        Ok(Self { child, pid, reader })
    }
    pub(super) async fn ready(
        &mut self,
        spec: &HelperSpec,
        address: SocketAddr,
        stderr: &Stderr,
    ) -> Result<(), HelperError> {
        let check = async {
            let mut poll = tokio::time::interval(Duration::from_millis(20));
            loop {
                tokio::select! {
                    status = self.child.wait() => {
                        self.drain().await;
                        return Err(spec.failure(format!("Exited before readiness: {status:?}"), stderr));
                    }
                    _ = poll.tick() => {
                        if TcpStream::connect(address).await.is_ok() { return Ok(()); }
                    }
                }
            }
        };
        match tokio::time::timeout(spec.timeout, check).await {
            Ok(result) => result,
            Err(_) => Err(spec.failure("Timed out waiting for the loopback port", stderr)),
        }
    }
    pub(super) async fn drain(&mut self) {
        if let Some(reader) = self.reader.take() {
            let _ = tokio::time::timeout(Duration::from_secs(1), reader).await;
        }
    }
    pub(super) async fn stop(&mut self) {
        kill_tree(self.pid);
        let _ = self.child.kill().await;
        let _ = self.child.wait().await;
        self.drain().await;
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        kill_tree(self.pid);
        let _ = self.child.start_kill();
        if let Some(reader) = &self.reader {
            reader.abort();
        }
    }
}

pub(super) fn kill_tree(pid: u32) {
    #[cfg(unix)]
    {
        if let Ok(pid) = i32::try_from(pid) {
            let _ = nix::sys::signal::killpg(
                nix::unistd::Pid::from_raw(pid),
                nix::sys::signal::Signal::SIGKILL,
            );
        }
    }
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .output();
    }
}

/// Reserve and release an ephemeral IPv4 loopback endpoint.
///
/// # Errors
/// The OS cannot allocate a local port.
pub fn local_endpoint() -> Result<SocketAddr, HelperError> {
    TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .and_then(|listener| listener.local_addr())
        .map_err(|error| HelperError::Invalid(format!("Cannot allocate a loopback port: {error}")))
}

// SPDX-License-Identifier: Apache-2.0
//! Isolated child processes: dedicated directories, a scrubbed environment and
//! whole-process-group termination on drop.
use crate::AiError;
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::Stdio,
};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

#[cfg(unix)]
const SYSTEM_PATH: [&str; 4] = ["/usr/bin", "/bin", "/usr/sbin", "/sbin"];
#[cfg(windows)]
const SYSTEM_PATH: [&str; 0] = [];

/// Find the configured absolute executable, or the named `codex` tool.
///
/// GUI apps start with a minimal `PATH`, so the tool's usual install
/// directories are probed after `PATH`. Nothing else is searched or read.
pub(crate) fn locate(configured: &str) -> Option<PathBuf> {
    if !configured.is_empty() {
        let path = PathBuf::from(configured);
        return executable(&path).then_some(path);
    }
    let names: &[&str] = if cfg!(windows) {
        &["codex.exe", "codex.cmd"]
    } else {
        &["codex"]
    };
    let mut directories: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|value| std::env::split_paths(&value).collect())
        .unwrap_or_default();
    if let Some(home) = std::env::var_os("HOME") {
        directories.push(PathBuf::from(home).join(".local/bin"));
    }
    directories.extend(["/opt/homebrew/bin", "/usr/local/bin"].map(PathBuf::from));
    directories
        .iter()
        .flat_map(|directory| names.iter().map(move |name| directory.join(name)))
        .find(|path| executable(path))
}
fn executable(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    #[cfg(unix)]
    let runnable = {
        use std::os::unix::fs::PermissionsExt as _;
        metadata.permissions().mode() & 0o111 != 0
    };
    #[cfg(not(unix))]
    let runnable = true;
    path.is_absolute() && metadata.is_file() && runnable
}

/// One located executable and its dedicated app-data root.
#[derive(Clone, Debug)]
pub(crate) struct Launch {
    executable: PathBuf,
    root: PathBuf,
}
impl Launch {
    pub(crate) fn new(executable: PathBuf, root: PathBuf) -> Self {
        Self { executable, root }
    }
    /// The empty working directory the CLI is confined to.
    pub(crate) fn work(&self) -> PathBuf {
        self.root.join("work")
    }
    fn home(&self) -> PathBuf {
        self.root.join("home")
    }
    fn temporary(&self) -> PathBuf {
        self.root.join("tmp")
    }
    fn prepare(&self) -> Result<(), AiError> {
        for directory in [self.home(), self.work(), self.temporary()] {
            std::fs::create_dir_all(&directory).map_err(|_| AiError::InvalidConfig)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt as _;
                std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))
                    .map_err(|_| AiError::InvalidConfig)?;
            }
        }
        Ok(())
    }
    /// A command with no inherited environment, stdin/stdout pipes and no stderr.
    pub(crate) fn command(&self, args: &[String]) -> Result<Command, AiError> {
        self.prepare()?;
        let mut command = Command::new(&self.executable);
        command
            .args(args)
            .env_clear()
            .envs(self.environment())
            .current_dir(self.work())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt as _;
            command.as_std_mut().process_group(0);
        }
        Ok(command)
    }
    /// Only non-secret locations: the CLI's state lives in the dedicated home,
    /// so the user's own Codex configuration, MCP servers and skills are not loaded.
    fn environment(&self) -> Vec<(&'static str, OsString)> {
        let mut path: Vec<PathBuf> = self
            .executable
            .parent()
            .map(Path::to_path_buf)
            .into_iter()
            .collect();
        if let Some(directory) = std::fs::canonicalize(&self.executable)
            .ok()
            .and_then(|real| real.parent().map(Path::to_path_buf))
        {
            path.push(directory);
        }
        path.extend(SYSTEM_PATH.map(PathBuf::from));
        let mut environment = vec![
            ("PATH", std::env::join_paths(path).unwrap_or_default()),
            ("HOME", self.home().into_os_string()),
            ("CODEX_HOME", self.home().into_os_string()),
            ("TMPDIR", self.temporary().into_os_string()),
            ("LANG", "en_US.UTF-8".into()),
        ];
        if cfg!(windows) {
            let system = ["SystemRoot", "windir"]
                .into_iter()
                .filter_map(|name| std::env::var_os(name).map(|value| (name, value)));
            environment.extend(system);
            for name in ["USERPROFILE", "APPDATA", "LOCALAPPDATA"] {
                environment.push((name, self.home().into_os_string()));
            }
            environment.push(("TEMP", self.temporary().into_os_string()));
        }
        environment
    }
}

/// A spawned child whose whole process group is killed when dropped.
pub(crate) struct Isolated {
    child: Child,
    pid: Option<u32>,
}
impl Isolated {
    pub(crate) fn spawn(mut command: Command) -> Result<(Self, ChildStdin, ChildStdout), AiError> {
        let mut child = command.spawn().map_err(|_| AiError::Unsupported)?;
        let pid = child.id();
        let stdin = child.stdin.take().ok_or(AiError::Network)?;
        let stdout = child.stdout.take().ok_or(AiError::Network)?;
        Ok((Self { child, pid }, stdin, stdout))
    }
    /// Kill the process group and reap the direct child.
    pub(crate) async fn stop(mut self) {
        kill_tree(self.pid.take());
        let _ = self.child.kill().await;
    }
}
impl Drop for Isolated {
    fn drop(&mut self) {
        kill_tree(self.pid.take());
        // Tokio reaps a killed child left behind by a dropped future.
        let _ = self.child.start_kill();
    }
}
fn kill_tree(pid: Option<u32>) {
    let Some(pid) = pid else {
        return;
    };
    #[cfg(unix)]
    if let Some(group) = i32::try_from(pid)
        .ok()
        .and_then(rustix::process::Pid::from_raw)
    {
        let _ = rustix::process::kill_process_group(group, rustix::process::Signal::KILL);
    }
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

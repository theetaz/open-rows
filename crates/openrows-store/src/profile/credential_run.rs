// SPDX-License-Identifier: Apache-2.0
//! Bounded, cancellable direct process execution; stdout and stderr are zeroized.
use super::{CommandCancellation, CommandTrust, CredentialCommand, CredentialError, ProfileId};
use openrows_secrets::SecretString;
use std::{
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};
use zeroize::Zeroizing;
const OUTPUT_LIMIT: usize = 65_536;

/// Run a trusted credential command on a worker thread, with closed stdin.
/// # Errors
/// Untrusted, timeout, cancellation, process failure or scrubbed nonzero exit.
pub fn run_credential(
    trust: &CommandTrust,
    id: ProfileId,
    command: &CredentialCommand,
    cancel: &CommandCancellation,
) -> Result<SecretString, CredentialError> {
    command.validate()?;
    trust.require(id, command)?;
    if cancel.is_cancelled() {
        return Err(CredentialError::Cancelled);
    }
    let mut child = configured(command)
        .spawn()
        .map_err(CredentialError::Process)?;
    let result = collect(&mut child, command, cancel);
    if result.is_err() {
        terminate(&mut child);
    }
    let _ = child.wait();
    result
}
fn configured(command: &CredentialCommand) -> Command {
    let mut process = Command::new(&command.program);
    process
        .args(&command.args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (name, _) in std::env::vars_os() {
        let upper = name.to_string_lossy().to_ascii_uppercase();
        if upper.starts_with("OPENROWS_")
            || matches!(upper.as_str(), "PGPASSWORD" | "PGSSLPASSWORD" | "MYSQL_PWD")
        {
            process.env_remove(name);
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        process.process_group(0);
    }
    process
}
fn collect(
    child: &mut Child,
    command: &CredentialCommand,
    cancel: &CommandCancellation,
) -> Result<SecretString, CredentialError> {
    let (sender, receiver) = mpsc::channel();
    capture(child.stdout.take(), sender.clone(), false);
    capture(child.stderr.take(), sender, true);
    let started = Instant::now();
    let mut stdout = None;
    let mut stderr = None;
    loop {
        if cancel.is_cancelled() {
            return Err(CredentialError::Cancelled);
        }
        if started.elapsed() >= Duration::from_secs(u64::from(command.timeout_seconds)) {
            return Err(CredentialError::Timeout);
        }
        if let Ok((is_stderr, bytes)) = receiver.recv_timeout(Duration::from_millis(20)) {
            if is_stderr {
                stderr = Some(bytes);
            } else {
                stdout = Some(bytes);
            }
        }
        let status = child.try_wait().map_err(CredentialError::Process)?;
        if let (Some(status), Some(output), Some(errors)) =
            (status, stdout.as_ref(), stderr.as_ref())
        {
            return finish(
                status.success(),
                output
                    .as_ref()
                    .map_err(|_| CredentialError::Failed("Cannot read stdout".into()))?,
                errors
                    .as_ref()
                    .map_err(|_| CredentialError::Failed("Cannot read stderr".into()))?,
            );
        }
    }
}
type Captured = Result<Zeroizing<Vec<u8>>, std::io::Error>;
fn capture<R: std::io::Read + Send + 'static>(
    pipe: Option<R>,
    sender: mpsc::Sender<(bool, Captured)>,
    stderr: bool,
) {
    std::thread::spawn(move || {
        let bytes = pipe.map_or_else(|| Ok(Zeroizing::new(Vec::new())), read_bounded);
        let _ = sender.send((stderr, bytes));
    });
}
fn read_bounded(mut pipe: impl std::io::Read) -> Captured {
    let mut bytes = Zeroizing::new(Vec::with_capacity(OUTPUT_LIMIT));
    let mut buffer = Zeroizing::new([0; 4096]);
    loop {
        let read = pipe.read(&mut buffer[..])?;
        if read == 0 {
            return Ok(bytes);
        }
        let kept = read.min(OUTPUT_LIMIT.saturating_sub(bytes.len()));
        bytes.extend_from_slice(&buffer[..kept]);
        // Continue draining without growing memory or blocking the child's pipes.
    }
}
fn finish(success: bool, stdout: &[u8], stderr: &[u8]) -> Result<SecretString, CredentialError> {
    if stdout.len() >= OUTPUT_LIMIT {
        return Err(CredentialError::Failed("Output limit exceeded".into()));
    }
    let utf8 = std::str::from_utf8(stdout)
        .map_err(|_| CredentialError::Failed("Output is not UTF-8".into()))?;
    let text = Zeroizing::new(utf8.to_owned());
    let first = text.lines().next().unwrap_or_default().trim();
    if success && !first.is_empty() && stdout.len() < OUTPUT_LIMIT {
        return Ok(SecretString::from(first));
    }
    let diagnostics = scrub(stderr, first);
    Err(CredentialError::Failed(if diagnostics.is_empty() {
        "No password produced or nonzero exit".into()
    } else {
        diagnostics
    }))
}
fn scrub(stderr: &[u8], secret: &str) -> String {
    let raw = Zeroizing::new(String::from_utf8_lossy(stderr).into_owned());
    let replaced = Zeroizing::new(if secret.is_empty() {
        raw.to_string()
    } else {
        raw.replace(secret, "[redacted]")
    });
    let mut safe = String::new();
    for line in replaced.lines() {
        let lower = Zeroizing::new(line.to_ascii_lowercase());
        let at = ["password=", "token="]
            .iter()
            .filter_map(|key| lower.find(key).map(|at| at + key.len()))
            .min();
        let part = at.map_or(line, |at| &line[..at]);
        safe.extend(part.chars().take(512_usize.saturating_sub(safe.len())));
        if at.is_some() {
            safe.push_str("[redacted]");
        }
        if safe.len() >= 512 {
            break;
        }
        safe.push('\n');
    }
    safe.trim().chars().take(512).collect()
}
fn terminate(child: &mut Child) {
    #[cfg(unix)]
    {
        let group = i32::try_from(child.id())
            .ok()
            .and_then(rustix::process::Pid::from_raw);
        if let Some(group) = group {
            let _ = rustix::process::kill_process_group(group, rustix::process::Signal::KILL);
        }
    }
    let _ = child.kill();
}

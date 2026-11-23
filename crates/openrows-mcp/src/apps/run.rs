// SPDX-License-Identifier: Apache-2.0
//! Run an agent CLI once: argv only (never a shell), the user's environment so
//! the tool finds its own configuration, a `PATH` that also covers the usual
//! install folders (Node-based CLIs need `node`), plain output, a deadline,
//! bounded output and the whole process group killed when it overruns.
use super::{AppError, search_directories};
use std::{path::Path, process::Stdio, time::Duration};
use tokio::io::{AsyncRead, AsyncReadExt};

/// Output kept per stream.
const LIMIT: u64 = 65_536;

/// A finished command: exit status and plain stdout followed by stderr.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Output {
    /// Zero exit status.
    pub success: bool,
    /// Combined text without terminal escape sequences.
    pub text: String,
}

/// Run `program args…` with a deadline.
/// # Errors
/// The program cannot start, or it overran the deadline.
pub async fn run(program: &Path, args: &[String], limit: Duration) -> Result<Output, AppError> {
    let mut command = tokio::process::Command::new(program);
    command
        .args(args)
        .current_dir(std::env::temp_dir())
        .env("PATH", path_for(program))
        .env("NO_COLOR", "1")
        .env("TERM", "dumb")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(unix)]
    command.process_group(0);
    let mut child = command.spawn().map_err(|_| AppError::NotInstalled)?;
    let mut group = Group(child.id());
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let work = async {
        let (out, err) = tokio::join!(bounded(stdout), bounded(stderr));
        let status = child.wait().await;
        (status, out, err)
    };
    let (status, out, err) = tokio::time::timeout(limit, work)
        .await
        .map_err(|_| AppError::Timeout)?;
    group.0 = None;
    let status = status.map_err(|_| AppError::NotInstalled)?;
    Ok(Output {
        success: status.success(),
        text: plain(&format!("{out}\n{err}")),
    })
}
/// The program's folder, the search folders, then the inherited `PATH`.
fn path_for(program: &Path) -> std::ffi::OsString {
    let mut folders: Vec<_> = program
        .parent()
        .map(Path::to_path_buf)
        .into_iter()
        .collect();
    folders.extend(search_directories());
    for system in ["/usr/bin", "/bin", "/usr/sbin", "/sbin"] {
        folders.push(system.into());
    }
    std::env::join_paths(folders).unwrap_or_default()
}
async fn bounded(stream: Option<impl AsyncRead + Unpin>) -> String {
    let mut bytes = Vec::new();
    if let Some(stream) = stream {
        // A read error leaves what was read; the exit status still decides.
        let _ = stream.take(LIMIT).read_to_end(&mut bytes).await;
    }
    String::from_utf8_lossy(&bytes).into_owned()
}
/// Remove ANSI escape sequences and other control characters except newlines.
pub(crate) fn plain(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        if character == '\u{1b}' {
            if characters.peek() == Some(&'[') {
                characters.next();
                characters
                    .by_ref()
                    .find(char::is_ascii_alphabetic)
                    .map(drop)
                    .unwrap_or_default();
            }
        } else if character == '\n' || character == '\t' || !character.is_control() {
            output.push(character);
        }
    }
    output
}
/// Kills the child's whole process group (tools may start helpers) unless disarmed.
struct Group(Option<u32>);
impl Drop for Group {
    fn drop(&mut self) {
        #[cfg(unix)]
        if let Some(group) = self
            .0
            .and_then(|pid| i32::try_from(pid).ok())
            .and_then(rustix::process::Pid::from_raw)
        {
            let _ = rustix::process::kill_process_group(group, rustix::process::Signal::KILL);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_drops_colour_and_controls() {
        assert_eq!(
            plain("\u{1b}[32m✓\u{1b}[39m openrows: x\r\n"),
            "✓ openrows: x\n"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn runs_argv_without_a_shell_and_enforces_the_deadline() -> Result<(), AppError> {
        let echo = Path::new("/bin/echo");
        let output = run(echo, &["a;b".to_owned()], Duration::from_secs(5)).await?;
        assert!(output.success);
        assert!(output.text.starts_with("a;b"));
        let slow = run(
            Path::new("/bin/sh"),
            &[
                "-c".to_owned(),
                "exec /bin/cat /dev/zero > /dev/null".to_owned(),
            ],
            Duration::from_millis(200),
        )
        .await;
        assert_eq!(slow, Err(AppError::Timeout));
        let missing = run(Path::new("/nonexistent/tool"), &[], Duration::from_secs(1)).await;
        assert_eq!(missing, Err(AppError::NotInstalled));
        Ok(())
    }
}

// SPDX-License-Identifier: Apache-2.0
//! Run a [`ToolCommand`] as a child process in its own process group:
//! stream its stderr as log lines, report progress (objects from
//! `--verbose`, bytes piped in or written), cancel by signalling the whole
//! group, and report how it ended. Blocking: run it on a thread of its own.
use std::fs::File;
use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::command::ToolCommand;
use crate::error::DumpError;

/// How often log lines and progress are reported, and cancel is checked.
const TICK: Duration = Duration::from_millis(200);

/// Cancels a run from any thread.
#[derive(Clone, Debug, Default)]
pub struct Canceller(Arc<AtomicBool>);

impl Canceller {
    /// Ask the run to stop; it signals the tool within one tick.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// Whether [`Self::cancel`] was called.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// What a running tool reported.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunEvent {
    /// New stderr lines, in order.
    Log(Vec<String>),
    /// Counts so far.
    Progress {
        /// Objects the tool reported as done.
        objects: u64,
        /// Bytes piped in, or written to the output.
        bytes: u64,
    },
}

/// How a run ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunOutcome {
    /// Exit status 0.
    Completed,
    /// Stopped through the [`Canceller`].
    Cancelled,
    /// A non-zero exit; `message` is the last error the tool printed.
    Failed {
        /// Exit code, absent when a signal ended the tool.
        code: Option<i32>,
        /// The tool's last error line, or the exit status.
        message: String,
    },
}

/// Run `command` to its end.
///
/// # Errors
/// The tool cannot be started or its input file cannot be opened.
pub fn run(
    command: &ToolCommand,
    cancel: &Canceller,
    mut on_event: impl FnMut(RunEvent),
) -> Result<RunOutcome, DumpError> {
    let input = command
        .stdin
        .as_deref()
        .map(|path| File::open(path).map_err(|error| DumpError::io("cannot read", path, error)))
        .transpose()?;
    let mut child = spawn(command)?;
    let fed = Arc::new(AtomicU64::new(0));
    let feeder = input.and_then(|file| feed(&mut child, file, Arc::clone(&fed)));
    let lines = read_lines(&mut child);
    let mut tracker = Tracker::new(command, fed);
    let mut killed = false;
    loop {
        match lines.recv_timeout(TICK) {
            Ok(line) => tracker.line(line),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        if cancel.is_cancelled() && !killed {
            terminate(&mut child);
            killed = true;
        }
        tracker.report(&mut on_event, false);
    }
    let status = child
        .wait()
        .map_err(|error| DumpError::io("cannot wait for", &command.program, error))?;
    if let Some(feeder) = feeder {
        // A failed copy shows as the tool's own error and exit status.
        let _ = feeder.join();
    }
    tracker.report(&mut on_event, true);
    Ok(outcome(status, cancel.is_cancelled(), tracker.last_error))
}

/// Run `command` (without input) and return its standard output, e.g.
/// `pg_restore --list`.
///
/// # Errors
/// The tool cannot be started, or fails; the error holds its last message.
pub fn capture(command: &ToolCommand) -> Result<String, DumpError> {
    let output = configured(command)
        .stdout(Stdio::piped())
        .output()
        .map_err(|error| DumpError::io("cannot start", &command.program, error))?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let message = stderr.lines().rev().find(|line| !line.trim().is_empty());
    Err(DumpError::invalid(message.map_or_else(
        || format!("The tool ended with {}", output.status),
        str::to_owned,
    )))
}

fn spawn(command: &ToolCommand) -> Result<Child, DumpError> {
    configured(command)
        .spawn()
        .map_err(|error| DumpError::io("cannot start", &command.program, error))
}

/// The process for `command`: arguments, environment, pipes, own group.
fn configured(command: &ToolCommand) -> Command {
    let mut process = Command::new(&command.program);
    process
        .args(&command.args)
        .envs(command.env.iter().map(|(name, value)| (name, value)))
        .stdin(if command.stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    for name in &command.env_remove {
        process.env_remove(name);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        // Its own group, so cancel also stops `-j` worker processes.
        process.process_group(0);
    }
    process
}

/// Copy `file` into the child's stdin on a thread, counting bytes.
fn feed(child: &mut Child, mut file: File, fed: Arc<AtomicU64>) -> Option<JoinHandle<()>> {
    let mut stdin = child.stdin.take()?;
    Some(std::thread::spawn(move || {
        let mut buffer = vec![0; 64 * 1024];
        loop {
            let read = match file.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(read) => read,
            };
            if stdin.write_all(&buffer[..read]).is_err() {
                break;
            }
            fed.fetch_add(read as u64, Ordering::Relaxed);
        }
        // Dropping stdin closes the pipe: the tool sees the end of input.
    }))
}

/// Stderr lines (lossily decoded) on a channel that closes at end of output.
fn read_lines(child: &mut Child) -> mpsc::Receiver<String> {
    let (sender, receiver) = mpsc::channel();
    if let Some(stderr) = child.stderr.take() {
        std::thread::spawn(move || forward_lines(stderr, &sender));
    }
    receiver
}

fn forward_lines(stderr: impl std::io::Read, sender: &mpsc::Sender<String>) {
    let mut reader = BufReader::new(stderr);
    let mut line = Vec::new();
    while reader
        .read_until(b'\n', &mut line)
        .is_ok_and(|read| read > 0)
    {
        let text = String::from_utf8_lossy(&line).trim_end().to_owned();
        line.clear();
        // A closed receiver means the run ended; stop reading.
        if sender.send(text).is_err() {
            break;
        }
    }
}

/// Stop the tool and every process it started.
fn terminate(child: &mut Child) {
    #[cfg(unix)]
    {
        let group = i32::try_from(child.id())
            .ok()
            .and_then(rustix::process::Pid::from_raw);
        if let Some(group) = group
            && rustix::process::kill_process_group(group, rustix::process::Signal::TERM).is_ok()
        {
            return;
        }
    }
    // Killing a process that already exited fails harmlessly.
    let _ = child.kill();
}

fn outcome(status: ExitStatus, cancelled: bool, last_error: Option<String>) -> RunOutcome {
    if cancelled {
        return RunOutcome::Cancelled;
    }
    if status.success() {
        return RunOutcome::Completed;
    }
    RunOutcome::Failed {
        code: status.code(),
        message: last_error.unwrap_or_else(|| format!("The tool ended with {status}")),
    }
}

/// Lines, counts and the last error since the previous report.
struct Tracker<'a> {
    command: &'a ToolCommand,
    fed: Arc<AtomicU64>,
    pending: Vec<String>,
    objects: u64,
    last_error: Option<String>,
    reported: Option<Instant>,
}

impl<'a> Tracker<'a> {
    fn new(command: &'a ToolCommand, fed: Arc<AtomicU64>) -> Self {
        Self {
            command,
            fed,
            pending: Vec::new(),
            objects: 0,
            last_error: None,
            reported: None,
        }
    }

    fn line(&mut self, line: String) {
        if self.command.progress.counts(&line) {
            self.objects += 1;
        }
        let lower = line.to_ascii_lowercase();
        if lower.contains("error") || lower.contains("fatal") {
            self.last_error = Some(line.clone());
        }
        self.pending.push(line);
    }

    fn bytes(&self) -> u64 {
        match &self.command.output {
            Some(output) if self.command.stdin.is_none() => size_of(output),
            _ => self.fed.load(Ordering::Relaxed),
        }
    }

    /// Report at most once per tick, and always at the end.
    fn report(&mut self, on_event: &mut impl FnMut(RunEvent), last: bool) {
        let due = self.reported.is_none_or(|at| at.elapsed() >= TICK);
        if !due && !last {
            return;
        }
        self.reported = Some(Instant::now());
        if !self.pending.is_empty() {
            on_event(RunEvent::Log(std::mem::take(&mut self.pending)));
        }
        on_event(RunEvent::Progress {
            objects: self.objects,
            bytes: self.bytes(),
        });
    }
}

/// Size of a file, or of the files directly in a folder.
pub(crate) fn size_of(path: &Path) -> u64 {
    let Ok(meta) = std::fs::metadata(path) else {
        return 0;
    };
    if !meta.is_dir() {
        return meta.len();
    }
    std::fs::read_dir(path)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter_map(|entry| entry.metadata().ok())
                .filter(std::fs::Metadata::is_file)
                .map(|meta| meta.len())
                .sum()
        })
        .unwrap_or(0)
}

#[cfg(test)]
#[path = "tests/run.rs"]
mod tests;

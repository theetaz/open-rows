// SPDX-License-Identifier: Apache-2.0
//! Find installed tools, ask each for its version and pick the copy that
//! best matches the server.
use std::cmp::Reverse;
use std::collections::HashSet;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use crate::error::DumpError;
use crate::tools::{ToolKind, is_executable};
use crate::version::{ServerVersion, ToolVersion, parse_tool_version};

/// Longest wait for `--version`; real tools answer in milliseconds.
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// An installed tool and the version it reported.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FoundTool {
    /// Which tool.
    pub kind: ToolKind,
    /// Where it is, as found (not resolved through symlinks).
    pub path: PathBuf,
    /// What its `--version` said.
    pub version: ToolVersion,
}

/// Every executable named like `kind` in `dirs`, in search order; copies
/// that resolve to the same file are listed once.
#[must_use]
pub fn candidates(kind: ToolKind, dirs: &[PathBuf]) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    let mut found = Vec::new();
    for dir in dirs {
        for name in kind.names() {
            let path = dir.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
            if !is_executable(&path) {
                continue;
            }
            let real = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
            if seen.insert(real) {
                found.push(path);
            }
        }
    }
    found
}

/// Run `path --version` and parse the answer.
///
/// # Errors
/// The program cannot run, does not answer within five seconds, or is not
/// the expected tool.
pub fn probe(kind: ToolKind, path: &Path) -> Result<FoundTool, DumpError> {
    if !is_executable(path) {
        return Err(DumpError::NotExecutable {
            path: path.to_path_buf(),
        });
    }
    let output = version_output(path)?;
    let wrong = || DumpError::WrongTool {
        path: path.to_path_buf(),
        tool: kind.label(),
        reported: output.lines().next().unwrap_or_default().to_owned(),
    };
    let version = parse_tool_version(&output).ok_or_else(wrong)?;
    if version.product.family() != kind.family() {
        return Err(wrong());
    }
    Ok(FoundTool {
        kind,
        path: path.to_path_buf(),
        version,
    })
}

/// Standard output of `path --version`, killed after [`PROBE_TIMEOUT`].
fn version_output(path: &Path) -> Result<String, DumpError> {
    let mut child = Command::new(path)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| DumpError::io("cannot run", path, error))?;
    let Some(mut stdout) = child.stdout.take() else {
        return Ok(String::new());
    };
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut text = String::new();
        let read = stdout.read_to_string(&mut text).map(|_| text);
        // The receiver is gone only after a timeout; the text is not needed then.
        let _ = sender.send(read);
    });
    let answer = receiver.recv_timeout(PROBE_TIMEOUT);
    if answer.is_err() {
        // Killing a process that already exited fails harmlessly.
        let _ = child.kill();
    }
    let _ = child.wait();
    match answer {
        Ok(read) => read.map_err(|error| DumpError::io("cannot read the version of", path, error)),
        Err(_) => Err(DumpError::io(
            "no answer to --version from",
            path,
            std::io::ErrorKind::TimedOut.into(),
        )),
    }
}

/// Every installed copy of `kind` in `dirs` that answers `--version`.
#[must_use]
pub fn discover(kind: ToolKind, dirs: &[PathBuf]) -> Vec<FoundTool> {
    candidates(kind, dirs)
        .iter()
        .filter_map(|path| probe(kind, path).ok())
        .collect()
}

/// The copy to use for `server`: the right product family, then a release
/// at least as new as the server's (the closest such), then the newest.
/// Without a server version the newest copy wins.
#[must_use]
pub fn choose(found: &[FoundTool], server: Option<ServerVersion>) -> Option<&FoundTool> {
    found.iter().rev().max_by_key(|tool| rank(tool, server))
}

fn rank(
    tool: &FoundTool,
    server: Option<ServerVersion>,
) -> (bool, bool, Reverse<u32>, crate::Version) {
    let version = tool.version.version;
    let Some(server) = server else {
        return (true, true, Reverse(0), version);
    };
    let same_product = tool.version.product == server.product;
    let new_enough = version.major >= server.version.major;
    let distance = if new_enough {
        version.major - server.version.major
    } else {
        0
    };
    (same_product, new_enough, Reverse(distance), version)
}

#[cfg(test)]
#[path = "tests/discover.rs"]
mod tests;

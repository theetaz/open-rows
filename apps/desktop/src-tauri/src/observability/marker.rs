// SPDX-License-Identifier: Apache-2.0
//! Crash detection (docs/14 §2.1). A marker is written at startup and removed
//! at a clean exit. Finding one whose owner is gone means the previous run
//! ended without exiting: a crash, a force quit or a power loss. A marker
//! owned by a live process is another instance, not a crash.
use std::fs;
use std::io::{self, ErrorKind};
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Marker file name in the app-data directory.
pub(crate) const FILE_NAME: &str = "run-marker.json";

#[derive(Serialize, Deserialize)]
struct Marker {
    pid: u32,
}

/// Claim the marker for this process; `true` when the previous run crashed.
///
/// # Errors
/// The marker cannot be read or written.
pub(crate) fn begin(path: &Path) -> io::Result<bool> {
    let own = std::process::id();
    let previous = match fs::read_to_string(path) {
        Ok(text) => Some(text),
        Err(error) if error.kind() == ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    if previous
        .as_deref()
        .is_some_and(|text| running_elsewhere(text, own))
    {
        // A live instance owns the marker and must keep it to detect its own crash.
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, owner_text(own)?)?;
    Ok(previous.is_some())
}

/// Remove the marker on a clean exit, but only when this process owns it.
///
/// # Errors
/// The marker exists but cannot be removed.
pub(crate) fn end(path: &Path) -> io::Result<()> {
    match fs::read_to_string(path) {
        Ok(text) if owner(&text) == Some(std::process::id()) => fs::remove_file(path),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn owner_text(pid: u32) -> io::Result<String> {
    serde_json::to_string(&Marker { pid }).map_err(io::Error::other)
}

fn owner(text: &str) -> Option<u32> {
    serde_json::from_str::<Marker>(text)
        .ok()
        .map(|marker| marker.pid)
}

/// Whether the marker names another process that is still alive.
fn running_elsewhere(text: &str, own: u32) -> bool {
    owner(text).is_some_and(|pid| pid != own && alive(pid))
}

#[cfg(unix)]
fn alive(pid: u32) -> bool {
    use rustix::io::Errno;
    use rustix::process::{Pid, test_kill_process};
    let Some(pid) = i32::try_from(pid).ok().and_then(Pid::from_raw) else {
        return false;
    };
    // EPERM means the process exists but belongs to another user.
    match test_kill_process(pid) {
        Ok(()) => true,
        Err(error) => error == Errno::PERM,
    }
}

#[cfg(not(unix))]
fn alive(_pid: u32) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Process 1 always exists and is owned by root, so it is never this test.
    const OTHER_LIVE_PID: u32 = 1;

    #[test]
    fn a_clean_exit_leaves_no_marker_and_the_next_start_is_not_a_crash()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("nested").join(FILE_NAME);
        assert!(!begin(&path)?);
        assert!(path.exists());
        end(&path)?;
        assert!(!path.exists());
        assert!(!begin(&path)?);
        Ok(())
    }

    #[test]
    fn a_marker_of_a_dead_process_means_the_last_run_crashed()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join(FILE_NAME);
        fs::write(&path, r#"{"pid":2147483646}"#)?;
        assert!(begin(&path)?);
        Ok(())
    }

    #[test]
    fn an_unreadable_marker_counts_as_a_crash() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join(FILE_NAME);
        fs::write(&path, "half written")?;
        assert!(begin(&path)?);
        Ok(())
    }

    #[test]
    fn a_live_other_instance_is_not_a_crash_and_its_marker_survives_our_exit()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join(FILE_NAME);
        fs::write(&path, format!(r#"{{"pid":{OTHER_LIVE_PID}}}"#))?;
        assert!(!begin(&path)?);
        end(&path)?;
        assert!(path.exists());
        Ok(())
    }
}

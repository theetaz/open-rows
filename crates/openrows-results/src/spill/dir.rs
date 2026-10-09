// SPDX-License-Identifier: Apache-2.0
//! The per-run spill directory and the startup sweep of earlier runs'.
//!
//! Each run owns `run-<pid>-<nanos>/` under the cache root and holds an
//! exclusive lock on its `.lock` file until it exits, so a directory whose
//! lock can be taken belongs to a run that ended (or crashed) and is swept.
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Prefix of every per-run directory.
const RUN_PREFIX: &str = "run-";
/// Lock file held for the lifetime of a run.
const LOCK_FILE: &str = ".lock";

/// A private (0700) directory for this run's spill files; removed on drop.
#[derive(Debug)]
pub struct SpillDir {
    path: PathBuf,
    /// Held, never read: the lock marks the directory as in use.
    _lock: File,
}

impl SpillDir {
    /// Sweep stale run directories under `root`, then create this run's.
    ///
    /// # Errors
    /// The root or the run directory cannot be created or locked.
    pub fn create(root: &Path) -> io::Result<Self> {
        private_dir(root, true)?;
        sweep(root);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos());
        let path = root.join(format!("{RUN_PREFIX}{}-{nanos}", std::process::id()));
        private_dir(&path, false)?;
        let lock = private_file(&path.join(LOCK_FILE))?;
        lock.try_lock()?;
        Ok(Self { path, _lock: lock })
    }

    /// The directory spill files are written to.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Delete the directory and every spill file in it.
    pub(crate) fn remove(&self) {
        // Already gone is fine; anything left is swept at the next start.
        let _ = fs::remove_dir_all(&self.path);
    }
}

impl Drop for SpillDir {
    fn drop(&mut self) {
        self.remove();
    }
}

/// Remove run directories under `root` whose run has ended; returns how many.
/// Best effort: an unreadable entry is left for the next start.
pub(crate) fn sweep(root: &Path) -> usize {
    let Ok(entries) = fs::read_dir(root) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().starts_with(RUN_PREFIX))
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter(|entry| is_stale(&entry.path()))
        .filter(|entry| fs::remove_dir_all(entry.path()).is_ok())
        .count()
}

/// No live run holds the directory's lock (or it never got one).
fn is_stale(dir: &Path) -> bool {
    match File::open(dir.join(LOCK_FILE)) {
        Ok(lock) => lock.try_lock().is_ok(),
        Err(error) => error.kind() == io::ErrorKind::NotFound,
    }
}

/// Create a directory only this user can enter.
fn private_dir(path: &Path, recursive: bool) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(recursive);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(path)
}

/// Create a new file only this user can read or write.
pub(crate) fn private_file(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)
}

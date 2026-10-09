// SPDX-License-Identifier: Apache-2.0
//! Run a `.sql` file too large for the editor (M2-12, FR-IO-03).
//!
//! `run_file_pick` shows the native open panel and remembers the chosen
//! path (only picked files can be checked or run); `run_file_check` streams
//! the file once through the safety gate of the connection; `run_file_start`
//! runs it on a session of its own with progress events, cancelled through
//! `query_cancel`.
pub(crate) mod commands;
mod dto;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use openrows_core::script_file::FileScan;

/// A file's last check, for the database it was checked against.
#[derive(Clone)]
struct Check {
    scope: String,
    scan: FileScan,
}

/// Picked files and their last checks.
#[derive(Default)]
pub(crate) struct RunFiles {
    picked: Mutex<HashSet<PathBuf>>,
    checks: Mutex<HashMap<PathBuf, Check>>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl RunFiles {
    fn pick(&self, path: PathBuf) {
        lock(&self.picked).insert(path);
    }

    fn is_picked(&self, path: &Path) -> bool {
        lock(&self.picked).contains(path)
    }

    fn checked(&self, path: PathBuf, check: Check) {
        lock(&self.checks).insert(path, check);
    }

    fn check(&self, path: &Path) -> Option<Check> {
        lock(&self.checks).get(path).cloned()
    }
}

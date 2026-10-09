// SPDX-License-Identifier: Apache-2.0
//! Import of CSV, TSV, JSON and NDJSON files into tables (M2-11, FR-IO-02,
//! FR-IO-06).
//!
//! `import_pick_file` shows the native open panel and keeps the path in the
//! core (the webview only holds a file id); `import_preview` detects the
//! parse options and returns inferred column types, the first rows and the
//! cells a target type would reject; `import_start` checks the mapping
//! against the table (or the new table's definition), opens a dedicated
//! connection and runs a dry run or the import in the background with
//! progress, cancel (`import_cancel`) and a final summary. Imports are
//! writes: read-only connections refuse them.
pub(crate) mod commands;
mod dto;
mod plan;
mod run;

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};

use openrows_import::CancellationToken;

/// Picked files remembered at once; older ids expire.
const MAX_FILES: usize = 16;

#[derive(Default)]
struct State {
    /// Picked files by id, oldest first.
    files: VecDeque<(String, PathBuf)>,
    /// Cancel tokens of running jobs by import id.
    running: HashMap<String, CancellationToken>,
}

/// Picked files and running imports.
#[derive(Default)]
pub(crate) struct Imports(Mutex<State>);

impl Imports {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn remember(&self, file_id: &str, path: PathBuf) {
        let mut state = self.lock();
        if state.files.len() >= MAX_FILES {
            state.files.pop_front();
        }
        state.files.push_back((file_id.to_owned(), path));
    }

    fn path(&self, file_id: &str) -> Option<PathBuf> {
        self.lock()
            .files
            .iter()
            .find(|(id, _)| id == file_id)
            .map(|(_, path)| path.clone())
    }

    fn track(&self, import_id: &str, cancel: CancellationToken) {
        self.lock().running.insert(import_id.to_owned(), cancel);
    }

    fn forget(&self, import_id: &str) {
        self.lock().running.remove(import_id);
    }

    fn cancel(&self, import_id: &str) {
        if let Some(cancel) = self.lock().running.get(import_id) {
            cancel.cancel();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_expire_and_jobs_cancel_by_id() {
        let imports = Imports::default();
        for index in 0..=MAX_FILES {
            imports.remember(&index.to_string(), PathBuf::from(format!("/f{index}")));
        }
        assert_eq!(imports.path("0"), None);
        assert_eq!(imports.path("1"), Some(PathBuf::from("/f1")));
        let cancel = CancellationToken::new();
        imports.track("job", cancel.clone());
        imports.cancel("other");
        assert!(!cancel.is_cancelled());
        imports.cancel("job");
        assert!(cancel.is_cancelled());
        imports.forget("job");
        assert!(imports.lock().running.is_empty());
    }
}

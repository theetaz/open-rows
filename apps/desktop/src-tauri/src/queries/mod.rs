// SPDX-License-Identifier: Apache-2.0
//! SQL execution exposed to the webview (D-07).
//!
//! `query_execute` splits the editor text with `openrows-sql`, gates it with
//! [`openrows_core::safety`] using the session's saved connection, then runs
//! the statements one by one on the session actor. Batches stream into the
//! shared result store (read through `result_window`); events go back over a
//! Tauri channel and each executed statement is written to the local history.
//! Editor placeholders are bound per [`params`]; [`saved`] keeps saved
//! queries and [`files`] saves and opens the `.sql` files of query tabs;
//! [`file_watch`] notices when another program changed one; [`explain`]
//! explains a statement into an analysed plan (FR-EDIT-07).
pub(crate) mod commands;
mod dto;
pub(crate) mod events;
pub(crate) mod explain;
pub(crate) mod explain_dto;
pub(crate) mod file_watch;
pub(crate) mod files;
pub(crate) mod history;
pub(crate) mod history_privacy;
mod params;
pub(crate) mod resolve;
pub(crate) mod saved;

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use openrows_core::script::ScriptCanceller;

/// Cancel handles of the runs still in progress, by query id.
#[derive(Default)]
pub(crate) struct Queries {
    running: Mutex<HashMap<String, ScriptCanceller>>,
}

impl Queries {
    pub(crate) fn track(&self, query_id: &str, cancel: ScriptCanceller) {
        self.lock().insert(query_id.to_owned(), cancel);
    }

    pub(crate) fn forget(&self, query_id: &str) {
        self.lock().remove(query_id);
    }

    fn cancel(&self, query_id: &str) {
        if let Some(cancel) = self.lock().get(query_id) {
            cancel.cancel();
        }
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<String, ScriptCanceller>> {
        self.running.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

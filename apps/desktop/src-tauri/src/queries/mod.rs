// SPDX-License-Identifier: Apache-2.0
//! SQL execution exposed to the webview (D-07).
//!
//! `query_execute` splits the editor text with `openrows-sql`, gates it with
//! [`openrows_core::safety`] using the session's saved connection, then runs
//! the statements one by one on the session actor. Batches stream into the
//! shared result store (read through `result_window`); events go back over a
//! Tauri channel and each executed statement is written to the local history.
pub(crate) mod commands;
mod dto;
mod events;
mod history;

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use openrows_core::script::ScriptCanceller;

/// Cancel handles of the runs still in progress, by query id.
#[derive(Default)]
pub(crate) struct Queries {
    running: Mutex<HashMap<String, ScriptCanceller>>,
}

impl Queries {
    fn track(&self, query_id: &str, cancel: ScriptCanceller) {
        self.lock().insert(query_id.to_owned(), cancel);
    }

    fn forget(&self, query_id: &str) {
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

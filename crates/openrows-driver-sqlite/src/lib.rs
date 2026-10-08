// SPDX-License-Identifier: Apache-2.0
//! `SQLite` driver (D-12, dogfood slice of M1-13).
//!
//! Opens a local database file (read-only, read-write, or created on
//! request) with `rusqlite`'s bundled `SQLite`, extension loading off. Each
//! connection lives on its own thread ([`worker`]); statements stream back
//! into Arrow batches whose column types follow the declared affinity and
//! the values seen, with tagged text for mixed columns ([`decode`]). Cancel
//! is `sqlite3_interrupt`. The [`catalog`] lists the attached databases as
//! schemas and reads tables, views, columns, keys, indexes and triggers
//! through the pragma functions; the [`editor`] snapshots rows as exact text
//! and commits staged edits in one `BEGIN IMMEDIATE` transaction.
#![forbid(unsafe_code)]
mod catalog;
mod connect;
mod decode;
mod editor;
mod error;
mod session;
mod stream;
mod worker;

pub use connect::{SqliteDriver, open};
pub use session::SqliteSession;

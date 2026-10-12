// SPDX-License-Identifier: Apache-2.0
//! `SQLite` driver (D-12, dogfood slice of M1-13).
//!
//! Opens a local database file (read-only, read-write, or created on
//! request) or a process-shared in-memory database with `rusqlite`'s
//! bundled `SQLite`, extension loading off, then attaches the profile's
//! other database files. Each
//! connection lives on its own thread ([`worker`]); statements stream back
//! into Arrow batches whose column types follow the declared affinity and
//! the values seen, with tagged text for mixed columns ([`decode`]). Cancel
//! is `sqlite3_interrupt`. The [`catalog`] lists the attached databases as
//! schemas and reads tables, views, columns, keys, indexes and triggers
//! through the pragma functions; the [`editor`] snapshots rows as exact text
//! and commits staged edits in one `BEGIN IMMEDIATE` transaction. Imports
//! append rows through one cached prepared `INSERT` ([`bulk`]). The
//! [`monitor`] reads own-session statements and cheap file diagnostics.
#![forbid(unsafe_code)]
mod bulk;
mod capabilities;
mod catalog;
pub mod catalog_definition;
pub mod catalog_sql;
mod connect;
mod ddl;
pub mod decode;
mod editor;
mod error;
mod libsql_file;
mod monitor;
mod session;
mod stream;
mod worker;

pub use connect::{SqliteDriver, open, open_source};
pub use libsql_file::LibsqlFileDriver;
pub use session::SqliteSession;

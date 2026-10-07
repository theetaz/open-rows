// SPDX-License-Identifier: Apache-2.0
//! `OpenRows` local persistence (docs/15).
//!
//! - [`LocalStore`]: the `SQLite` app database (WAL, numbered embedded
//!   migrations, backup before migrating) holding `meta` and query history.
//! - [`profile`]: `PostgreSQL` connection profiles stored as secret-free JSON
//!   files (`connections/<id>.json`, docs/15 §3), the `postgres://` URL
//!   parser and [`profile::ProfileService`], which keeps passwords in a
//!   [`openrows_secrets::SecretStore`] and only a reference on disk.

#![forbid(unsafe_code)]

mod db;
pub mod profile;

pub use db::{
    HistoryEntry, HistoryRecord, HistorySource, HistoryStatus, LocalStore, SCHEMA_VERSION,
    StoreError,
};

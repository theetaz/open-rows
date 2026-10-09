// SPDX-License-Identifier: Apache-2.0
//! `OpenRows` local persistence (docs/15).
//!
//! - [`LocalStore`]: the `SQLite` app database (WAL, numbered embedded
//!   migrations, backup before migrating) holding `meta`, query history and
//!   the journal of staged table edits (`change_sets`) and saved queries.
//! - [`profile`]: `PostgreSQL` connection profiles stored as secret-free JSON
//!   files (`connections/<id>.json`, docs/15 §3), the `postgres://` URL
//!   parser and [`profile::ProfileService`], which keeps passwords in a
//!   [`openrows_secrets::SecretStore`] and only a reference on disk.

#![forbid(unsafe_code)]

mod db;
pub mod profile;

pub use db::{
    ChangeSetRecord, HistoryEntry, HistoryRecord, HistorySource, HistoryStatus, LocalStore,
    NewSavedQuery, SCHEMA_VERSION, SavedQueryRecord, StoreError,
};

// SPDX-License-Identifier: Apache-2.0
//! `OpenRows` local persistence (docs/15).
//!
//! - [`LocalStore`]: the `SQLite` app database (WAL, numbered embedded
//!   migrations, backup before migrating) holding `meta`, query history and
//!   the journal of staged table edits (`change_sets`), saved queries and
//!   schema-cache snapshots (`schema_snapshots`) and per-window session and
//!   workspace state (`workspace_state`: query tabs, open connections).
//! - [`profile`]: `PostgreSQL` connection profiles stored as secret-free JSON
//!   files (`connections/<id>.json`, docs/15 §3), the `postgres://` URL
//!   parser and [`profile::ProfileService`], which keeps passwords in a
//!   [`openrows_secrets::SecretStore`] and only a reference on disk.

#![forbid(unsafe_code)]

mod db;
pub mod profile;

pub use db::{
    ChangeSetRecord, HistoryEntry, HistoryRecord, HistoryRetention, HistorySource, HistoryStatus,
    LocalStore, MonitorAggregate, NewSavedQuery, SCHEMA_VERSION, SavedQueryRecord,
    SchemaSnapshotRecord, StoreError, WorkspaceKey,
};

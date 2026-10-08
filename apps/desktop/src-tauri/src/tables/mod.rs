// SPDX-License-Identifier: Apache-2.0
//! Table browsing exposed to the webview (D-08, docs/08 §10).
//!
//! `table_page` and `table_count` generate parameterised `SELECT`s with
//! `openrows_sql::select` after validating every identifier against the
//! relation's catalog columns (read on the metadata session), then run them
//! on the target database's session with bind parameters. Pages stream into the
//! shared result store like query results; both are cancelled through
//! `query_cancel`. `table_structure` serves the Data tab's columns and
//! estimate; `table_detail` serves the Structure, Index, Triggers and DDL
//! views and `table_alter` reviews and commits structure changes.
//! `table_edit_info`, `table_edit_row(s)` and `table_edit` stage and commit
//! row edits; `table_edits_save/saved/clear` journal staged edits locally so
//! they survive a crash (docs/10 §4.5, NFR-REL-01).
pub(crate) mod commands;
mod dto;

pub(crate) mod edit_commands;
mod edit_dto;
pub(crate) mod file_value;
pub(crate) mod recovery_commands;
pub(crate) mod schema_commands;
mod schema_dto;
mod schema_map;
pub(crate) mod snapshot_commands;

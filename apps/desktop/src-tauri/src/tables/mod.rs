// SPDX-License-Identifier: Apache-2.0
//! Table browsing exposed to the webview (D-08, docs/08 §10).
//!
//! `table_page` and `table_count` generate parameterised `SELECT`s with
//! `openrows_sql::select` after validating every identifier against the
//! relation's catalog columns (read on the metadata session), then run them
//! on the target database's session with bind parameters. Pages stream into the
//! shared result store like query results; both are cancelled through
//! `query_cancel`. `table_structure` serves the Structure and Indexes tabs.
pub(crate) mod commands;
mod dto;

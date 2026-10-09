// SPDX-License-Identifier: Apache-2.0
//! Export of query results and whole tables to CSV, JSON and SQL `INSERT`
//! files (M1-25, FR-IO-01).
//!
//! `export_start` resolves the rows (a result in the shared store, one
//! read-only statement run again, or `SELECT *` of a table), shows the
//! native save panel, then streams through `openrows_core::export` in the
//! background. Rows never cross IPC: the webview only receives progress and
//! the terminal event over a channel, and cancels through `query_cancel`.
pub(crate) mod commands;
mod dto;

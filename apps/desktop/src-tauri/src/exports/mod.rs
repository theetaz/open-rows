// SPDX-License-Identifier: Apache-2.0
//! Export of query results, selected rows, tables and whole schemas to CSV,
//! TSV, JSON, NDJSON, SQL `INSERT`, Markdown, Excel and Parquet files
//! (M1-25, M2-13, FR-IO-01, FR-IO-05).
//!
//! `export_start` resolves the rows (a result in the shared store or some of
//! its rows, one read-only statement run again, or `SELECT *` of one or
//! more tables), shows the native save (or folder) panel, then streams
//! through `openrows_core::export` as a background job. Statements run on a
//! session of the job's own and stored results stay pinned while read, so
//! jobs run in parallel and outlive the tab that started them. Rows never
//! cross IPC: the webview receives progress and the terminal event over a
//! channel, cancels through `query_cancel` and opens or reveals the output
//! through [`outputs`].
pub(crate) mod commands;
mod dto;
mod format_dto;
pub(crate) mod outputs;
pub(crate) mod pins;
mod plan;
mod run;
mod selected;

pub(crate) use dto::capitalised;

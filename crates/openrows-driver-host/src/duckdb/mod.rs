// SPDX-License-Identifier: Apache-2.0
//! Host-only native `DuckDB` connection; the independent control runtime interrupts it.
mod bridge;
mod catalog;
mod catalog_sql;
mod connect;
mod detail;
mod editor;
mod error;
mod nested;
mod session;
mod stream;
mod text;
pub(crate) use connect::connect;

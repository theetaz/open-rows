// SPDX-License-Identifier: Apache-2.0
//! Streamed export of result batches to files (FR-IO-01, docs/06 §8).
//!
//! An [`Exporter`] encodes Arrow batches as CSV (RFC 4180), JSON (an array
//! of objects or NDJSON) or dialect-correct SQL `INSERT` statements and
//! writes them to a temporary file next to the destination. Only
//! [`Exporter::finish`] renames it into place; dropping an unfinished
//! exporter (error or cancel) deletes the temporary file, so a partial
//! export never replaces or impersonates the chosen file.
//!
//! Everything here is synchronous and free of async runtimes: callers run
//! [`drain`] or [`write_all`] on a blocking thread and feed it batches from
//! a driver stream or from a complete in-memory result.
#![forbid(unsafe_code)]
mod cell;
mod csv;
mod error;
mod exporter;
mod file;
mod format;
mod json;
mod pipeline;
mod sql;
#[cfg(test)]
mod tests;
mod text;

pub use error::ExportError;
pub use exporter::Exporter;
pub use format::{CsvOptions, Delimiter, Format, InsertOptions, JsonLayout};
pub use pipeline::{Message, drain, write_all};

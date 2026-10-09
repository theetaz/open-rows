// SPDX-License-Identifier: Apache-2.0
//! Streamed import of CSV, TSV, JSON and NDJSON files into tables
//! (FR-IO-02, FR-IO-06).
//!
//! [`inspect`] detects a file's encoding, format, delimiter and header,
//! names its columns, infers their [`TypeCategory`] over a sample and keeps
//! the first rows for the mapping preview. [`spawn_feed`] then reads every
//! row on a blocking thread, maps and checks it with a [`Mapper`] and sends
//! bounded batches; [`load`] writes them on a dedicated connection through
//! the driver's bulk loader (`COPY FROM STDIN`, a prepared `INSERT`) or
//! multi-row parameterised `INSERT`s with the chosen conflict policy, in one
//! transaction or one per batch, and reports progress, row errors with line
//! numbers and a final [`Summary`]. A dry run ([`RunMode::Check`]) reads
//! and checks everything and counts rows whose key already exists.
#![forbid(unsafe_code)]
mod decode;
mod detect;
mod error;
mod feed;
mod infer;
mod load;
mod mapping;
mod merge;
mod options;
mod read;
mod runner;
mod source;
mod sql;
mod temporal;
mod types;

pub use error::{ImportError, RowError};
pub use feed::{Batch, Chunk, FeedSource, spawn_feed};
pub use infer::ColumnStats;
pub use load::{
    BadRows, Control, Failure, LoadPlan, MAX_REPORTED_ERRORS, Outcome, RunMode, Summary,
    TransactionMode, load,
};
pub use mapping::{ColumnSource, MappedColumn, Mapper};
pub use options::{FileFormat, ParseOptions};
pub use source::{Inspection, Item, Record, SampleLimits, SourceColumn, inspect, stream};
pub use sql::{ConflictPolicy, NewColumn, TableName, create_table_sql};
pub use tokio_util::sync::CancellationToken;
pub use types::TypeCategory;

#[cfg(test)]
#[path = "tests/files.rs"]
mod files_tests;

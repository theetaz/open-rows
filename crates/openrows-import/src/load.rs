// SPDX-License-Identifier: Apache-2.0
//! The loading side of an import (FR-IO-02, FR-IO-06), on a dedicated
//! connection: create and truncate the table, then write each batch through
//! the driver's bulk loader (plain appends) or multi-row parameterised
//! `INSERT`s (conflict clauses, drivers without one). A dry run instead
//! counts rows whose key already exists, writing nothing.
//!
//! Transactions: `Single` wraps everything in one transaction, so a failure
//! or cancel leaves the table as it was (a `MySQL` table created by the
//! import is dropped again, as its DDL commits implicitly). `PerBatch`
//! commits each batch; a failure keeps the batches before it.
use openrows_driver_api::{Connection, DriverError};
use openrows_sql::Dialect;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::feed::Chunk;
use crate::runner::Runner;
use crate::sql::{ConflictPolicy, TableName};
use crate::{ImportError, RowError};

/// Row errors kept for the report; the count includes the rest.
pub const MAX_REPORTED_ERRORS: usize = 100;

/// When work is committed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransactionMode {
    /// One transaction for the whole import: all or nothing.
    Single,
    /// One transaction per batch: a failure keeps the earlier batches.
    PerBatch,
}

/// What a malformed row or rejected value does.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BadRows {
    /// Stop the import (nothing of the batch holding the row is written).
    Abort,
    /// Report the row and go on.
    Skip,
}

/// Write the rows, or only check them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunMode {
    /// Dry run: read and check every row and count existing keys.
    Check,
    /// Write the rows.
    Import,
}

/// Everything the loader does, decided before it starts.
#[derive(Clone, Debug)]
pub struct LoadPlan {
    /// The target's SQL dialect.
    pub dialect: Dialect,
    /// The target table.
    pub table: TableName,
    /// Target columns in row order (the mapper's targets).
    pub columns: Vec<String>,
    /// Columns identifying existing rows (conflict policy and dry run).
    pub key: Vec<String>,
    /// What happens to rows whose key exists.
    pub conflict: ConflictPolicy,
    /// `CREATE TABLE` to run first, for a new table.
    pub create: Option<String>,
    /// Empty the table before loading.
    pub truncate: bool,
    /// When work is committed.
    pub transaction: TransactionMode,
    /// What a bad row does.
    pub bad_rows: BadRows,
    /// Write or only check.
    pub mode: RunMode,
}

/// Counts so far; final once the load returns.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Summary {
    /// Rows read from the file, good and bad.
    pub rows: u64,
    /// Rows written (inserted or, with `Update`, upserted); after a rollback
    /// only the rows that stayed committed.
    pub inserted: u64,
    /// Rows skipped because their key existed (`Skip`).
    pub skipped: u64,
    /// Malformed rows and rows with rejected values.
    pub failed: u64,
    /// Dry run: rows whose key exists in the table; `None` when it cannot
    /// be told (new table, no key, key columns not mapped).
    pub conflicts: Option<u64>,
    /// The first [`MAX_REPORTED_ERRORS`] row errors.
    pub errors: Vec<RowError>,
}

/// Why a load failed.
#[derive(Debug)]
pub enum Failure {
    /// The server rejected a batch spanning these lines.
    Batch {
        /// The server's error.
        error: DriverError,
        /// First and last line of the batch.
        lines: Option<(u64, u64)>,
    },
    /// Creating, truncating or starting a transaction failed.
    Setup(DriverError),
    /// `COMMIT` failed; when its reply was lost the outcome is unknown.
    Commit(DriverError),
    /// The file could not be read further, or the plan cannot produce SQL.
    File(ImportError),
    /// A bad row stopped the import ([`BadRows::Abort`]); it is the last
    /// entry of [`Summary::errors`] unless that list was full.
    BadRow,
}

/// How a load ended.
#[derive(Debug)]
pub enum Outcome {
    /// Every row was handled and committed (or checked).
    Finished(Summary),
    /// The user cancelled; uncommitted work was rolled back.
    Cancelled(Summary),
    /// The load failed; uncommitted work was rolled back.
    Failed(Failure, Summary),
}

/// Cancellation and progress of a running load.
pub struct Control {
    /// Cancels the load and the statement it is running.
    pub cancel: CancellationToken,
    /// Called after every batch.
    pub progress: Box<dyn FnMut(&Summary) + Send>,
}

/// Run `plan` on `connection` with the batches of `feed` until the file
/// ends, a batch fails or the load is cancelled.
pub async fn load(
    connection: &mut dyn Connection,
    plan: &LoadPlan,
    feed: mpsc::Receiver<Chunk>,
    control: Control,
) -> Outcome {
    match Runner::new(connection, plan, control.cancel) {
        Ok(runner) => runner.run(feed, control.progress).await,
        Err(error) => Outcome::Failed(Failure::File(error), Summary::default()),
    }
}

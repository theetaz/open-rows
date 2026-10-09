// SPDX-License-Identifier: Apache-2.0
//! Bulk loading of rows into one table (FR-IO-02, docs/05 §2): the engine's
//! fastest append path, such as `PostgreSQL` `COPY FROM STDIN` or a `SQLite`
//! prepared insert reused for every row. Drivers without one fall back to
//! batched parameterised `INSERT`s in the importer.
use crate::{BoxFuture, DriverError};

/// One row as text cells in [`BulkTarget::columns`] order; `None` is NULL.
/// The server converts each text with the input rules of the column's type.
pub type TextRow = Vec<Option<String>>;

/// The table rows are appended to and the columns each row fills.
#[derive(Clone, Copy, Debug)]
pub struct BulkTarget<'a> {
    /// Schema (`PostgreSQL`), database (`MySQL`) or attached file (`SQLite`).
    pub schema: &'a str,
    /// Table name, unquoted.
    pub table: &'a str,
    /// Column names, unquoted; columns not listed get their defaults.
    pub columns: &'a [String],
}

/// Append-only bulk loading on a connection (the [`crate::Feature::BulkLoad`] facet).
pub trait BulkLoader: Send {
    /// Append `rows` to the target and return how many the server stored.
    ///
    /// Runs inside the connection's open transaction when there is one, so
    /// the caller decides between one transaction and per-batch commits. A
    /// failing row fails the whole call; nothing of it is kept outside a
    /// transaction the caller rolls back.
    ///
    /// # Errors
    /// `Server` for rejected rows (the connection stays usable),
    /// `Unsupported` for values the protocol cannot carry (such as NUL
    /// characters), transport and protocol failures.
    fn load<'a>(
        &'a mut self,
        target: BulkTarget<'a>,
        rows: Vec<TextRow>,
    ) -> BoxFuture<'a, Result<u64, DriverError>>;
}

// SPDX-License-Identifier: Apache-2.0
//! Pure SQL toolkit for `OpenRows`: statement splitting, current-statement
//! detection, safety classification, placeholder detection and the
//! parameterised table-browsing `SELECT`s of [`select`].
//!
//! Everything here is synchronous, allocation-light and free of I/O. No
//! function panics on any input (only [`select`] refuses invalid names with
//! a typed error); byte offsets always refer to the
//! original text so the editor can map results back to the document.
//!
//! ```
//! use openrows_sql::{Dialect, StatementKind, classify, split, statement_at};
//!
//! let sql = "SELECT 1; -- note\nDELETE FROM users;";
//! let statements = split(sql, Dialect::Postgres);
//! assert_eq!(statements.len(), 2);
//!
//! let current = statement_at(&statements, sql.len()).and_then(|i| statements.get(i));
//! let delete = current.map(|statement| statement.text(sql)).unwrap_or_default();
//! assert_eq!(delete, "DELETE FROM users");
//!
//! let class = classify(delete, Dialect::Postgres);
//! assert_eq!(class.kind, StatementKind::Write);
//! assert!(class.dangerous);
//! ```
#![forbid(unsafe_code)]

pub mod attach;
mod bind;
mod classify;
mod context;
mod dialect;
pub mod edit;
mod lexer;
mod placeholders;
pub mod select;
mod split;

#[cfg(test)]
mod tests;

pub use bind::{MissingParam, ParamBinder, ParamSlot, ParamStyle};
pub use classify::{Classification, StatementKind, classify};
pub use context::{ContextSetting, session_context};
pub use placeholders::{Placeholder, PlaceholderKind, placeholders};
pub use split::{Statement, split, statement_at};

/// SQL dialect whose lexical, classification and generation rules apply.
///
/// Further dialects (SQL Server `GO`, …) are added as new variants with
/// their own lexer rules.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum Dialect {
    /// `PostgreSQL` (and wire-compatible servers).
    #[default]
    Postgres,
    /// `MySQL` and `MariaDB` (default `sql_mode`: backslash escapes in strings,
    /// `"…"` strings, `` `…` `` identifiers, `#` comments, `DELIMITER`).
    MySql,
    /// `SQLite` 3.
    Sqlite,
}

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

mod ai_read;
pub use ai_read::{UnsafeAiSql, ai_read_source, validate_ai_read};

pub mod attach;
mod bind;
mod classify;
mod context;
pub mod ddl;
mod dialect;
pub mod duckdb;
pub mod edit;
mod explain;
mod lexer;
mod limit;
mod placeholders;
mod redact;
pub mod select;
mod split;

#[cfg(test)]
mod tests;

pub use bind::{MissingParam, ParamBinder, ParamSlot, ParamStyle};
pub use classify::{Classification, StatementKind, classify};
pub use context::{ContextSetting, session_context};
pub use explain::sqlserver_explainable;
pub use limit::limit_select;
pub use placeholders::{Placeholder, PlaceholderKind, placeholders};
pub use redact::{
    REDACTED_PREFIX, has_redacted_literals, literal_ranges, normalize_monitor_query,
    redact_identifiers, redact_literals,
};
pub use split::{ScriptStatement, Statement, StreamSplitter, split, statement_at};

/// SQL dialect whose lexical, classification and generation rules apply.
///
/// Further dialects are added as new variants with their own lexer rules.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum Dialect {
    /// `DuckDB`: `PostgreSQL` quoting, dollar strings and numbered parameters.
    Duckdb,
    /// `PostgreSQL` (and wire-compatible servers).
    #[default]
    Postgres,
    /// `MySQL` and `MariaDB` (default `sql_mode`: backslash escapes in strings,
    /// `"…"` strings, `` `…` `` identifiers, `#` comments, `DELIMITER`).
    MySql,
    /// `SQLite` 3.
    Sqlite,
    /// SQL Server and Azure SQL (T-SQL): `[…]` and `"…"` identifiers,
    /// `N'…'` strings, nested block comments, `@variables`, `#temp` names,
    /// `@P1` bind placeholders and the `GO` batch separator line.
    SqlServer,
    /// `ClickHouse` HTTP SQL, with typed `{name:Type}` query parameters.
    ClickHouse,
}

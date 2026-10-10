// SPDX-License-Identifier: Apache-2.0
//! Pure `EXPLAIN` support for `OpenRows` (FR-EDIT-07, docs/09 §6).
//!
//! [`command`] builds the statements that explain one statement on each
//! engine; [`parse`] turns the engine's output rows into the common
//! [`Plan`] model; [`analyse`] flattens it with exclusive time and cost per
//! operator and flags hotspots. No I/O: the core runs the statements.
//!
//! ```
//! use openrows_explain::{Engine, Mode, Wrap, analyse, command, parse};
//!
//! let explain = command(Engine::Sqlite, Mode::Plan, "SELECT * FROM t", Wrap::None);
//! assert_eq!(explain.explain, "EXPLAIN QUERY PLAN SELECT * FROM t");
//! let rows = vec![vec![Some("2".into()), Some("0".into()), Some("0".into()), Some("SCAN t".into())]];
//! let plan = analyse(parse(explain.format, &rows)?);
//! assert_eq!(plan.nodes[0].data.relation.as_deref(), Some("t"));
//! # Ok::<(), openrows_explain::ParseError>(())
//! ```
#![forbid(unsafe_code)]
mod analysis;
mod command;
mod json;
mod model;
mod mysql_json;
mod mysql_tree;
mod pg;
mod sqlite;
mod sqlserver;

pub use analysis::{
    AnalysedNode, AnalysedPlan, EXPENSIVE_SHARE, EXPENSIVE_TOP, Hotspot, LARGE_TABLE_ROWS,
    MISESTIMATE_RATIO, Weight, analyse,
};
pub use command::{AnalyzeWrites, Command, Mode, SAVEPOINT, Wrap, command};
pub use model::{Buffers, Detail, Engine, Format, NodeData, Plan, PlanNode};

/// Why plan output could not be read.
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    /// The JSON formats did not hold valid JSON.
    #[error("the plan is not valid JSON: {0}")]
    Json(#[from] serde_json::Error),
    /// Invalid or over-budget showplan XML.
    #[error("invalid showplan XML: {0}")]
    Xml(String),
    /// A required part was absent.
    #[error("the plan output has no {0}")]
    Missing(&'static str),
}

/// Parse the rows of an explaining statement (text cells in column order).
///
/// # Errors
/// Output that is not in `format`.
pub fn parse(format: Format, rows: &[Vec<Option<String>>]) -> Result<Plan, ParseError> {
    match format {
        Format::PostgresJson => pg::parse(&raw_text(format, rows)),
        Format::MySqlJson => mysql_json::parse(&raw_text(format, rows)),
        Format::MySqlTree => mysql_tree::parse(&raw_text(format, rows)),
        Format::SqliteRows => sqlite::parse(rows),
        Format::SqlServerXml => sqlserver::parse(&raw_text(format, rows)),
    }
}

/// The output as the engine's own client shows it: the JSON or tree text,
/// or `SQLite`'s indented query plan.
#[must_use]
pub fn raw_text(format: Format, rows: &[Vec<Option<String>>]) -> String {
    if format == Format::SqliteRows {
        return sqlite::render(rows);
    }
    rows.iter()
        .filter_map(|row| row.first().cloned().flatten())
        .collect::<Vec<_>>()
        .join("\n")
}

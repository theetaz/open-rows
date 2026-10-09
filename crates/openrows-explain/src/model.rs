// SPDX-License-Identifier: Apache-2.0
//! The engine-neutral plan model every parser produces (docs/09 §6).
//!
//! Fields follow `PostgreSQL`'s per-loop semantics: `actual_rows` and
//! `actual_total_ms` are averages per loop, so a node's whole time is
//! `actual_total_ms × loops`. Engines without a value leave it `None`. The
//! model is deliberately generic (operator name, object, estimates, actuals,
//! conditions and free-form details) so SQL Server showplan XML (`RelOp`
//! with `PhysicalOp`, `EstimateRows`, `RunTimeInformation`) can map onto it
//! with one more parser.

/// Database engine whose `EXPLAIN` dialect applies.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Engine {
    /// `PostgreSQL` and wire-compatible servers.
    Postgres,
    /// `MySQL` 8.
    MySql,
    /// `MariaDB`.
    MariaDb,
    /// `SQLite` 3.
    Sqlite,
}

/// How an engine's plan output is encoded; selects the parser.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Format {
    /// `EXPLAIN (FORMAT JSON)`: one row, one JSON array.
    PostgresJson,
    /// `MySQL` `EXPLAIN FORMAT=JSON` and `MariaDB` `EXPLAIN`/`ANALYZE FORMAT=JSON`.
    MySqlJson,
    /// `MySQL` `EXPLAIN ANALYZE`: one row of indented `->` tree text.
    MySqlTree,
    /// `SQLite` `EXPLAIN QUERY PLAN`: rows of `id`, `parent`, `notused`, `detail`.
    SqliteRows,
}

/// One labelled value shown in the node details (a condition, a sort key…).
#[derive(Clone, Debug, PartialEq)]
pub struct Detail {
    /// Engine's name for the value, e.g. `Filter` or `Sort Key`.
    pub label: String,
    /// The value as text.
    pub value: String,
}

impl Detail {
    /// A detail from any label and value.
    #[must_use]
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
        }
    }
}

/// Block counts of one node, including its children (`PostgreSQL` `BUFFERS`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Buffers {
    /// Shared blocks found in the buffer cache.
    pub shared_hit: f64,
    /// Shared blocks read from disk or the OS cache.
    pub shared_read: f64,
    /// Shared blocks this node dirtied.
    pub shared_dirtied: f64,
    /// Shared blocks this node wrote out.
    pub shared_written: f64,
    /// Temporary blocks read (sorts and hashes that spilled).
    pub temp_read: f64,
    /// Temporary blocks written (sorts and hashes that spilled).
    pub temp_written: f64,
}

/// What one plan operator reported; everything but its children.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NodeData {
    /// Operator, e.g. `Seq Scan`, `Hash Join`, `Index lookup`.
    pub node_type: String,
    /// Table (or other relation) the operator reads or writes.
    pub relation: Option<String>,
    /// Index the operator uses.
    pub index: Option<String>,
    /// Planner's row estimate per loop.
    pub estimated_rows: Option<f64>,
    /// Rows returned per loop (analysed plans).
    pub actual_rows: Option<f64>,
    /// How many times the operator ran (analysed plans).
    pub loops: Option<f64>,
    /// Planner's cost before the first row.
    pub startup_cost: Option<f64>,
    /// Planner's cost for all rows, including the children.
    pub total_cost: Option<f64>,
    /// Time to the first row per loop, in milliseconds.
    pub actual_startup_ms: Option<f64>,
    /// Time for all rows per loop, including the children, in milliseconds.
    pub actual_total_ms: Option<f64>,
    /// Rows a full scan reads from its table (all loops), when known.
    pub examined_rows: Option<f64>,
    /// The operator reads a whole table rather than an index range.
    pub full_scan: bool,
    /// Why the operator went to disk (a sort or hash that spilled).
    pub spill: Option<String>,
    /// Buffer usage, when the engine reported it.
    pub buffers: Option<Buffers>,
    /// Filters and join, index or sort conditions.
    pub conditions: Vec<Detail>,
    /// Every other reported value.
    pub details: Vec<Detail>,
}

impl NodeData {
    /// A node of `node_type` with nothing else known yet.
    #[must_use]
    pub fn new(node_type: impl Into<String>) -> Self {
        Self {
            node_type: node_type.into(),
            ..Self::default()
        }
    }
}

/// A plan operator and the operators feeding it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlanNode {
    /// The operator itself.
    pub data: NodeData,
    /// Inputs, in the engine's order.
    pub children: Vec<PlanNode>,
}

impl PlanNode {
    /// A node without children.
    #[must_use]
    pub fn leaf(data: NodeData) -> Self {
        Self {
            data,
            children: Vec::new(),
        }
    }
}

/// A parsed plan: one root operator and the run's totals.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    /// The top operator (a synthetic `Query plan` node when the engine
    /// reported several top-level operators).
    pub root: PlanNode,
    /// The statement ran and actual rows and times are present.
    pub analyzed: bool,
    /// Planning time in milliseconds, when reported.
    pub planning_ms: Option<f64>,
    /// Execution time in milliseconds, when reported.
    pub execution_ms: Option<f64>,
    /// Plan-wide values: settings, triggers, JIT…
    pub summary: Vec<Detail>,
}

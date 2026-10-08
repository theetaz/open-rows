// SPDX-License-Identifier: Apache-2.0
//! Schema-browser object model (docs/10 §2): the subset the D-06 tree lists
//! and the table structure the D-08 table view shows.

/// Kinds of catalog objects a driver lists below a schema.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ObjectKind {
    /// Ordinary or partitioned table.
    Table,
    /// View.
    View,
    /// Materialized view.
    MaterializedView,
    /// Foreign table.
    ForeignTable,
    /// Function (including window functions).
    Function,
    /// Stored procedure.
    Procedure,
    /// Sequence.
    Sequence,
}

impl ObjectKind {
    /// Every listable kind, in tree order.
    pub const ALL: [Self; 7] = [
        Self::Table,
        Self::ForeignTable,
        Self::View,
        Self::MaterializedView,
        Self::Function,
        Self::Procedure,
        Self::Sequence,
    ];

    /// Stable lower-case name used by catalog queries, e.g. `materialized_view`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Table => "table",
            Self::View => "view",
            Self::MaterializedView => "materialized_view",
            Self::ForeignTable => "foreign_table",
            Self::Function => "function",
            Self::Procedure => "procedure",
            Self::Sequence => "sequence",
        }
    }

    /// Inverse of [`ObjectKind::as_str`].
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == name)
    }
}

/// A database on the server.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DatabaseSummary {
    /// Database name.
    pub name: String,
    /// Maintenance or provider database (e.g. `postgres`, `rdsadmin`), hidden by default.
    pub system: bool,
    /// Comment, when set.
    pub comment: Option<String>,
}

/// A schema (namespace) inside a database.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchemaSummary {
    /// Schema name.
    pub name: String,
    /// Catalog schema (`pg_catalog`, `information_schema`), hidden by default.
    pub system: bool,
    /// Comment, when set.
    pub comment: Option<String>,
}

/// One listed object; the path is `[database, schema, name]`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectSummary {
    /// Owning schema.
    pub schema: String,
    /// Object name (without arguments).
    pub name: String,
    /// Object kind.
    pub kind: ObjectKind,
    /// Identity arguments of a routine, e.g. `integer, text`; `None` otherwise.
    pub arguments: Option<String>,
    /// Planner row estimate (not an exact count); `None` when unknown.
    pub row_estimate: Option<u64>,
    /// Comment, when set.
    pub comment: Option<String>,
}

/// One column of a table-like relation, in ordinal order (docs/10 §2).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ColumnSummary {
    /// Column name.
    pub name: String,
    /// Formatted type, e.g. `character varying(40)` or `timestamp with time zone`.
    pub data_type: String,
    /// `false` for `NOT NULL` columns.
    pub nullable: bool,
    /// Default or generation expression, when set.
    pub default: Option<String>,
    /// Comment, when set.
    pub comment: Option<String>,
    /// Part of the primary key.
    pub primary_key: bool,
}

/// One index of a table or materialized view.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexSummary {
    /// Index name.
    pub name: String,
    /// Key columns or expressions, comma-separated, in key order.
    pub columns: String,
    /// Enforces uniqueness.
    pub unique: bool,
    /// Backs the primary key.
    pub primary: bool,
    /// Access method, e.g. `btree` or `gin`.
    pub method: String,
    /// Partial-index predicate, when set.
    pub predicate: Option<String>,
}

/// A browsed relation's columns, indexes and planner row estimate.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TableStructure {
    /// Columns in ordinal order; empty when the relation does not exist.
    pub columns: Vec<ColumnSummary>,
    /// Indexes ordered by name.
    pub indexes: Vec<IndexSummary>,
    /// Planner estimate (`reltuples`); `None` when never analysed or not a table.
    pub row_estimate: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_names_round_trip() {
        for kind in ObjectKind::ALL {
            assert_eq!(ObjectKind::parse(kind.as_str()), Some(kind));
        }
        assert_eq!(ObjectKind::parse("index"), None);
    }
}

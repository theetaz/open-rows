// SPDX-License-Identifier: Apache-2.0
//! Table-browsing `SELECT`s with server-side filters, sort and pagination
//! (FR-GRID-04/08/11, docs/08 §10).
//!
//! Identifiers are quoted for the [`Dialect`] and must name a column the
//! catalog listed for the table; user values never enter the SQL text — they
//! become bind parameters (`$1`, `?` or `?1`) sent as text, so the server
//! converts them to the column's type. `LIMIT` and `OFFSET` are integers
//! formatted by Rust.
mod groups;
mod predicate;
mod raw;

use std::fmt::{self, Write as _};

use crate::Dialect;

/// The relation being browsed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TableRef {
    /// Schema (namespace) name.
    pub schema: String,
    /// Table, view, materialized view or foreign table name.
    pub name: String,
}

/// Comparison of a filter row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FilterOp {
    /// `=`
    Equal,
    /// `<>`
    NotEqual,
    /// `<`
    Less,
    /// `>`
    Greater,
    /// `<=`
    LessOrEqual,
    /// `>=`
    GreaterOrEqual,
    /// Case-insensitive substring match on the text form.
    Contains,
    /// Negated [`FilterOp::Contains`].
    NotContains,
    /// Case-insensitive prefix match on the text form.
    StartsWith,
    /// Case-insensitive suffix match on the text form.
    EndsWith,
    /// `IN` over a comma-separated value list.
    In,
    /// `IS NULL` (the value is ignored).
    IsNull,
    /// `IS NOT NULL` (the value is ignored).
    IsNotNull,
    /// `BETWEEN value AND second`.
    Between,
    /// Negated IN list.
    NotIn,
    /// Negated inclusive range.
    NotBetween,
    /// Case-insensitive SQL pattern, with explicit wildcards.
    Like,
    /// Negated SQL pattern.
    NotLike,
    /// Regular expression (`PostgreSQL` `~`, `MySQL` `REGEXP`; not `SQLite`).
    Regex,
    /// Negated regular expression.
    NotRegex,
    /// Empty string (not NULL).
    IsEmpty,
    /// Non-empty string (not NULL).
    IsNotEmpty,
    /// A raw SQL predicate typed by the user (`value`), checked by the
    /// lexer to be one balanced boolean expression without comments,
    /// terminators or placeholders; the column is ignored.
    Raw,
}

/// One enabled filter row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Filter {
    /// Column name as listed by the catalog.
    pub column: String,
    /// Comparison.
    pub op: FilterOp,
    /// Operand; a comma-separated list for [`FilterOp::In`].
    pub value: String,
    /// Upper bound for [`FilterOp::Between`]; ignored otherwise.
    pub second: String,
}

/// How filter rows combine.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Combinator {
    /// Every filter must match.
    #[default]
    And,
    /// Any filter may match.
    Or,
}

/// A parenthesised set of filter rows, combined before the outer join.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FilterGroup {
    /// Enabled rows of this group.
    pub filters: Vec<Filter>,
    /// Join between rows inside this group.
    pub combinator: Combinator,
}

/// Sort direction of the ordered column.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SortDirection {
    /// Ascending (the server's default NULL placement).
    Ascending,
    /// Descending (the server's default NULL placement).
    Descending,
}

/// Server-side `ORDER BY` on one column.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Sort {
    /// Column name as listed by the catalog.
    pub column: String,
    /// Direction.
    pub direction: SortDirection,
}

/// What one table page shows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TableQuery {
    /// The browsed relation.
    pub table: TableRef,
    /// Projected columns in order; empty selects every column (`*`).
    pub columns: Vec<String>,
    /// Enabled filters.
    pub filters: Vec<Filter>,
    /// Parenthesised groups, joined with the outer combinator.
    pub groups: Vec<FilterGroup>,
    /// How the filters and groups combine.
    pub combinator: Combinator,
    /// Case-insensitive search across every catalog column, AND-ed with filters.
    pub search: Option<String>,
    /// Optional sort.
    pub sort: Option<Sort>,
    /// A keyless `SQLite` table's rowid alias (`rowid`, `_rowid_` or `oid`),
    /// selected as the last column under its own name so staged edits can
    /// identify rows; `count_sql` ignores it.
    pub row_id: Option<String>,
}

/// One page of a table browse.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Page {
    /// Most rows to return.
    pub limit: u32,
    /// Rows to skip.
    pub offset: u64,
}

/// SQL text plus its positional text parameters (`None` is SQL NULL).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoundSql {
    /// Statement text with the dialect's placeholders.
    pub sql: String,
    /// Parameter values in placeholder order.
    pub params: Vec<Option<String>>,
}

/// Why a table query cannot be generated.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SelectError {
    /// The column is not one of the table's catalog columns.
    UnknownColumn(String),
    /// Identifiers and values cannot contain NUL characters.
    NulCharacter,
    /// An `IN` filter on this column has no values.
    EmptyList(String),
    /// The catalog listed no columns, so nothing can be validated.
    NoColumns,
    /// The dialect cannot express this request.
    Unsupported(&'static str),
    /// A raw SQL predicate is not one self-contained expression.
    InvalidPredicate(&'static str),
}

impl fmt::Display for SelectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownColumn(name) => write!(f, "the table has no column named {name:?}"),
            Self::NulCharacter => f.write_str("names and values cannot contain NUL characters"),
            Self::EmptyList(name) => {
                write!(f, "the IN filter on {name:?} needs at least one value")
            }
            Self::NoColumns => f.write_str("the table has no columns to select"),
            Self::Unsupported(what) => f.write_str(what),
            Self::InvalidPredicate(why) => write!(f, "the raw SQL filter {why}"),
        }
    }
}

impl std::error::Error for SelectError {}

/// `SELECT … LIMIT limit OFFSET offset` for one page; SQL Server pages
/// with `ORDER BY … OFFSET offset ROWS FETCH NEXT limit ROWS ONLY`, ordered
/// by `(SELECT NULL)` (no particular order) when no sort is set.
///
/// # Errors
/// See [`SelectError`]; `known` are the table's catalog column names.
pub fn page_sql(
    dialect: Dialect,
    query: &TableQuery,
    known: &[String],
    page: Page,
) -> Result<BoundSql, SelectError> {
    let projection = projection(dialect, query, known)?;
    let mut bound = select(dialect, query, known, &projection)?;
    if let Some(sort) = &query.sort {
        let direction = match sort.direction {
            SortDirection::Ascending => "ASC",
            SortDirection::Descending => "DESC",
        };
        let column = predicate::column(dialect, &sort.column, known)?;
        let _ = write!(bound.sql, " ORDER BY {column} {direction}");
    }
    if dialect == Dialect::SqlServer {
        sql_server_page(&mut bound.sql, query.sort.is_some(), page);
        return Ok(bound);
    }
    // Writing to a `String` cannot fail.
    let _ = write!(bound.sql, " LIMIT {} OFFSET {}", page.limit, page.offset);
    Ok(bound)
}

/// `OFFSET … FETCH` needs an `ORDER BY` and at least one row; an empty page
/// is `TOP (0)`.
fn sql_server_page(sql: &mut String, sorted: bool, page: Page) {
    if page.limit == 0 {
        sql.replace_range(.."SELECT ".len(), "SELECT TOP (0) ");
        return;
    }
    if !sorted {
        sql.push_str(" ORDER BY (SELECT NULL)");
    }
    let _ = write!(
        sql,
        " OFFSET {} ROWS FETCH NEXT {} ROWS ONLY",
        page.offset, page.limit
    );
}

/// `SELECT count(*)` over the same relation and filters (sort and paging ignored).
///
/// # Errors
/// As [`page_sql`].
pub fn count_sql(
    dialect: Dialect,
    query: &TableQuery,
    known: &[String],
) -> Result<BoundSql, SelectError> {
    // `COUNT(*)` is an `int` in SQL Server and overflows past 2^31 rows.
    let count = if dialect == Dialect::SqlServer {
        "COUNT_BIG(*)"
    } else {
        "count(*)"
    };
    select(dialect, query, known, count)
}

/// `SELECT * FROM table`: every row of a relation, unpaged (whole-table export).
///
/// # Errors
/// As [`page_sql`]; `known` must list the table's columns (an existing table).
pub fn all_rows_sql(
    dialect: Dialect,
    table: &TableRef,
    known: &[String],
) -> Result<BoundSql, SelectError> {
    let query = TableQuery {
        table: table.clone(),
        columns: Vec::new(),
        filters: Vec::new(),
        groups: Vec::new(),
        combinator: Combinator::And,
        search: None,
        sort: None,
        row_id: None,
    };
    select(dialect, &query, known, "*")
}
/// `SQLite`'s names for the rowid.
const ROW_ID_ALIASES: [&str; 3] = ["rowid", "_rowid_", "oid"];

fn projection(
    dialect: Dialect,
    query: &TableQuery,
    known: &[String],
) -> Result<String, SelectError> {
    let mut projection = if query.columns.is_empty() {
        "*".to_owned()
    } else {
        query
            .columns
            .iter()
            .map(|name| predicate::column(dialect, name, known))
            .collect::<Result<Vec<_>, _>>()?
            .join(", ")
    };
    if let Some(name) = &query.row_id {
        let quoted = row_id(dialect, name, known)?;
        let _ = write!(projection, ", {quoted} AS {quoted}");
    }
    Ok(projection)
}

/// The quoted rowid alias: `SQLite` only, never a real column's name.
fn row_id(dialect: Dialect, name: &str, known: &[String]) -> Result<String, SelectError> {
    if dialect != Dialect::Sqlite {
        return Err(SelectError::Unsupported("only SQLite tables have a rowid"));
    }
    let shadowed = known.iter().any(|column| column.eq_ignore_ascii_case(name));
    if shadowed || !ROW_ID_ALIASES.contains(&name) {
        return Err(SelectError::UnknownColumn(name.to_owned()));
    }
    dialect.quote_ident(name)
}

fn select(
    dialect: Dialect,
    query: &TableQuery,
    known: &[String],
    projection: &str,
) -> Result<BoundSql, SelectError> {
    if known.is_empty() {
        return Err(SelectError::NoColumns);
    }
    let table = dialect.qualified(&query.table.schema, &query.table.name)?;
    let mut params = Vec::new();
    let predicates = groups::where_predicate(dialect, query, known, &mut params)?;
    let mut sql = format!("SELECT {projection} FROM {table}");
    if !predicates.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&predicates);
    }
    Ok(BoundSql { sql, params })
}

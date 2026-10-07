// SPDX-License-Identifier: Apache-2.0
//! Table-browsing `SELECT`s with server-side filters, sort and pagination
//! (FR-GRID-04/08/11, docs/08 §10).
//!
//! Identifiers are double-quoted and must name a column the catalog listed
//! for the table; user values never enter the SQL text — they become
//! positional bind parameters (`$1`, `$2`, …) sent in text format, so the
//! server parses them with the column's own input function. `LIMIT` and
//! `OFFSET` are integers formatted by Rust.
use std::fmt::{self, Write as _};

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
    /// `PostgreSQL` regular expression.
    Regex,
    /// Negated regular expression.
    NotRegex,
    /// Empty string (not NULL).
    IsEmpty,
    /// Non-empty string (not NULL).
    IsNotEmpty,
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

/// Sort direction of the ordered column.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SortDirection {
    /// Ascending (`NULLS LAST`, the `PostgreSQL` default).
    Ascending,
    /// Descending (`NULLS FIRST`, the `PostgreSQL` default).
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
    /// How the filters combine.
    pub combinator: Combinator,
    /// Optional sort.
    pub sort: Option<Sort>,
}

/// SQL text plus its positional text parameters (`None` is SQL NULL).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoundSql {
    /// Statement text with `$n` placeholders.
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
        }
    }
}

impl std::error::Error for SelectError {}

/// A double-quoted `PostgreSQL` identifier (embedded quotes doubled).
///
/// # Errors
/// [`SelectError::NulCharacter`] when the name contains NUL.
pub fn quote_ident(name: &str) -> Result<String, SelectError> {
    if name.contains('\0') {
        return Err(SelectError::NulCharacter);
    }
    Ok(format!("\"{}\"", name.replace('"', "\"\"")))
}

/// `SELECT … LIMIT limit OFFSET offset` for one page.
///
/// # Errors
/// See [`SelectError`]; `known` are the table's catalog column names.
pub fn page_sql(
    query: &TableQuery,
    known: &[String],
    limit: u32,
    offset: u64,
) -> Result<BoundSql, SelectError> {
    let mut bound = select(query, known, &projection(query, known)?)?;
    if let Some(sort) = &query.sort {
        let direction = match sort.direction {
            SortDirection::Ascending => "ASC",
            SortDirection::Descending => "DESC",
        };
        let column = column(&sort.column, known)?;
        let _ = write!(bound.sql, " ORDER BY {column} {direction}");
    }
    // Writing to a `String` cannot fail.
    let _ = write!(bound.sql, " LIMIT {limit} OFFSET {offset}");
    Ok(bound)
}

/// `SELECT count(*)` over the same relation and filters (sort and paging ignored).
///
/// # Errors
/// As [`page_sql`].
pub fn count_sql(query: &TableQuery, known: &[String]) -> Result<BoundSql, SelectError> {
    select(query, known, "count(*)")
}

fn projection(query: &TableQuery, known: &[String]) -> Result<String, SelectError> {
    if query.columns.is_empty() {
        return Ok("*".to_owned());
    }
    let columns = query
        .columns
        .iter()
        .map(|name| column(name, known))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(columns.join(", "))
}

fn select(query: &TableQuery, known: &[String], projection: &str) -> Result<BoundSql, SelectError> {
    if known.is_empty() {
        return Err(SelectError::NoColumns);
    }
    let table = format!(
        "{}.{}",
        quote_ident(&query.table.schema)?,
        quote_ident(&query.table.name)?
    );
    let mut params = Vec::new();
    let predicates = query
        .filters
        .iter()
        .map(|filter| predicate(filter, known, &mut params))
        .collect::<Result<Vec<_>, _>>()?;
    let mut sql = format!("SELECT {projection} FROM {table}");
    if !predicates.is_empty() {
        let joiner = match query.combinator {
            Combinator::And => " AND ",
            Combinator::Or => " OR ",
        };
        sql.push_str(" WHERE ");
        sql.push_str(&predicates.join(joiner));
    }
    Ok(BoundSql { sql, params })
}

/// A catalog column, quoted; anything else is refused.
fn column(name: &str, known: &[String]) -> Result<String, SelectError> {
    if known.iter().any(|candidate| candidate == name) {
        quote_ident(name)
    } else {
        Err(SelectError::UnknownColumn(name.to_owned()))
    }
}

/// Append a parameter and return its placeholder.
fn bind(params: &mut Vec<Option<String>>, value: String) -> Result<String, SelectError> {
    if value.contains('\0') {
        return Err(SelectError::NulCharacter);
    }
    params.push(Some(value));
    Ok(format!("${}", params.len()))
}

fn predicate(
    filter: &Filter,
    known: &[String],
    params: &mut Vec<Option<String>>,
) -> Result<String, SelectError> {
    let column = column(&filter.column, known)?;
    let value = || filter.value.clone();
    let text = match filter.op {
        FilterOp::IsNull => format!("{column} IS NULL"),
        FilterOp::IsNotNull => format!("{column} IS NOT NULL"),
        FilterOp::In | FilterOp::NotIn => in_list(&column, filter, params)?,
        FilterOp::IsEmpty => format!("{column}::text = ''"),
        FilterOp::IsNotEmpty => format!("{column}::text <> ''"),
        FilterOp::Like | FilterOp::NotLike | FilterOp::Regex | FilterOp::NotRegex => {
            let operator = match filter.op {
                FilterOp::Like => "ILIKE",
                FilterOp::NotLike => "NOT ILIKE",
                FilterOp::Regex => "~",
                _ => "!~",
            };
            format!("{column}::text {operator} {}", bind(params, value())?)
        }
        FilterOp::Between | FilterOp::NotBetween => {
            let low = bind(params, value())?;
            let high = bind(params, filter.second.clone())?;
            let operator = if filter.op == FilterOp::NotBetween {
                "NOT BETWEEN"
            } else {
                "BETWEEN"
            };
            format!("{column} {operator} {low} AND {high}")
        }
        FilterOp::Contains | FilterOp::NotContains | FilterOp::StartsWith | FilterOp::EndsWith => {
            pattern(&column, filter, params)?
        }
        comparison => {
            let operator = comparison_operator(comparison);
            format!("{column} {operator} {}", bind(params, value())?)
        }
    };
    Ok(format!("({text})"))
}

fn comparison_operator(op: FilterOp) -> &'static str {
    match op {
        FilterOp::NotEqual => "<>",
        FilterOp::Less => "<",
        FilterOp::Greater => ">",
        FilterOp::LessOrEqual => "<=",
        FilterOp::GreaterOrEqual => ">=",
        _ => "=",
    }
}

fn in_list(
    column: &str,
    filter: &Filter,
    params: &mut Vec<Option<String>>,
) -> Result<String, SelectError> {
    let items: Vec<&str> = filter
        .value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .collect();
    if items.is_empty() {
        return Err(SelectError::EmptyList(filter.column.clone()));
    }
    let placeholders = items
        .into_iter()
        .map(|item| bind(params, item.to_owned()))
        .collect::<Result<Vec<_>, _>>()?;
    let operator = if filter.op == FilterOp::NotIn {
        "NOT IN"
    } else {
        "IN"
    };
    Ok(format!("{column} {operator} ({})", placeholders.join(", ")))
}

/// `ILIKE` on the column's text form; `%`, `_` and `\` in the value match literally.
fn pattern(
    column: &str,
    filter: &Filter,
    params: &mut Vec<Option<String>>,
) -> Result<String, SelectError> {
    let escaped = escape_like(&filter.value);
    let (value, negate) = match filter.op {
        FilterOp::StartsWith => (format!("{escaped}%"), false),
        FilterOp::EndsWith => (format!("%{escaped}"), false),
        FilterOp::NotContains => (format!("%{escaped}%"), true),
        _ => (format!("%{escaped}%"), false),
    };
    let operator = if negate { "NOT ILIKE" } else { "ILIKE" };
    let placeholder = bind(params, value)?;
    Ok(format!("{column}::text {operator} {placeholder}"))
}

/// Escape `LIKE` wildcards with the default escape character `\`.
fn escape_like(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        if matches!(character, '\\' | '%' | '_') {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

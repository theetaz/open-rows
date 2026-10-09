// SPDX-License-Identifier: Apache-2.0
//! One filter row as a parenthesised predicate with bound values.

use super::{Filter, FilterOp, SelectError};
use crate::Dialect;

/// A catalog column, quoted; anything else is refused.
pub(super) fn column(
    dialect: Dialect,
    name: &str,
    known: &[String],
) -> Result<String, SelectError> {
    if known.iter().any(|candidate| candidate == name) {
        dialect.quote_ident(name)
    } else {
        Err(SelectError::UnknownColumn(name.to_owned()))
    }
}

/// Append a parameter and return its placeholder.
fn bind(
    dialect: Dialect,
    params: &mut Vec<Option<String>>,
    value: String,
) -> Result<String, SelectError> {
    if value.contains('\0') {
        return Err(SelectError::NulCharacter);
    }
    params.push(Some(value));
    Ok(dialect.placeholder(params.len()))
}

pub(super) fn predicate(
    dialect: Dialect,
    filter: &Filter,
    known: &[String],
    params: &mut Vec<Option<String>>,
) -> Result<String, SelectError> {
    if filter.op == FilterOp::Raw {
        return super::raw::predicate(dialect, &filter.value);
    }
    let column = column(dialect, &filter.column, known)?;
    let value = || filter.value.clone();
    let text = match filter.op {
        FilterOp::IsNull => format!("{column} IS NULL"),
        FilterOp::IsNotNull => format!("{column} IS NOT NULL"),
        FilterOp::In | FilterOp::NotIn => in_list(dialect, &column, filter, params)?,
        FilterOp::IsEmpty => format!("{} = ''", dialect.text_of(&column)),
        FilterOp::IsNotEmpty => format!("{} <> ''", dialect.text_of(&column)),
        FilterOp::Like | FilterOp::NotLike => {
            let placeholder = bind(dialect, params, value())?;
            dialect.like(&column, &placeholder, filter.op == FilterOp::NotLike)
        }
        FilterOp::Regex | FilterOp::NotRegex => {
            let placeholder = bind(dialect, params, value())?;
            dialect.regex(&column, &placeholder, filter.op == FilterOp::NotRegex)?
        }
        FilterOp::Between | FilterOp::NotBetween => {
            let low = bind(dialect, params, value())?;
            let high = bind(dialect, params, filter.second.clone())?;
            let operator = if filter.op == FilterOp::NotBetween {
                "NOT BETWEEN"
            } else {
                "BETWEEN"
            };
            format!("{column} {operator} {low} AND {high}")
        }
        FilterOp::Contains | FilterOp::NotContains | FilterOp::StartsWith | FilterOp::EndsWith => {
            pattern(dialect, &column, filter, params)?
        }
        comparison => {
            let operator = comparison_operator(comparison);
            format!("{column} {operator} {}", bind(dialect, params, value())?)
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
    dialect: Dialect,
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
        .map(|item| bind(dialect, params, item.to_owned()))
        .collect::<Result<Vec<_>, _>>()?;
    let operator = if filter.op == FilterOp::NotIn {
        "NOT IN"
    } else {
        "IN"
    };
    Ok(format!("{column} {operator} ({})", placeholders.join(", ")))
}

/// Case-insensitive pattern on the column's text form; `%`, `_` and `\` in
/// the value match literally.
fn pattern(
    dialect: Dialect,
    column: &str,
    filter: &Filter,
    params: &mut Vec<Option<String>>,
) -> Result<String, SelectError> {
    let escaped = escape_like(dialect, &filter.value);
    let (value, negate) = match filter.op {
        FilterOp::StartsWith => (format!("{escaped}%"), false),
        FilterOp::EndsWith => (format!("%{escaped}"), false),
        FilterOp::NotContains => (format!("%{escaped}%"), true),
        _ => (format!("%{escaped}%"), false),
    };
    let placeholder = bind(dialect, params, value)?;
    Ok(dialect.like(column, &placeholder, negate))
}

/// Escape `LIKE` wildcards with `\` (SQL Server's `[` set too).
fn escape_like(dialect: Dialect, value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        let bracket = character == '[' && dialect == Dialect::SqlServer;
        if bracket || matches!(character, '\\' | '%' | '_') {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

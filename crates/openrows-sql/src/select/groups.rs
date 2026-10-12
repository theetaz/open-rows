// SPDX-License-Identifier: Apache-2.0
//! Parenthesised filter groups share one bind-parameter sequence.
use super::{Combinator, Filter, FilterOp, SelectError, TableQuery, predicate};
use crate::Dialect;

pub(super) fn predicates(
    dialect: Dialect,
    query: &TableQuery,
    known: &[String],
    params: &mut Vec<Option<String>>,
) -> Result<Vec<String>, SelectError> {
    let mut expressions = query
        .filters
        .iter()
        .map(|filter| predicate::predicate(dialect, filter, known, params))
        .collect::<Result<Vec<_>, _>>()?;
    for group in &query.groups {
        let rows = group
            .filters
            .iter()
            .map(|filter| predicate::predicate(dialect, filter, known, params))
            .collect::<Result<Vec<_>, _>>()?;
        if !rows.is_empty() {
            let join = match group.combinator {
                Combinator::And => " AND ",
                Combinator::Or => " OR ",
            };
            expressions.push(format!("({})", rows.join(join)));
        }
    }
    Ok(expressions)
}

/// Search is an independent AND term, preserving the existing outer group join.
pub(super) fn where_predicate(
    dialect: Dialect,
    query: &TableQuery,
    known: &[String],
    params: &mut Vec<Option<String>>,
) -> Result<String, SelectError> {
    let rows = predicates(dialect, query, known, params)?;
    let join = match query.combinator {
        Combinator::And => " AND ",
        Combinator::Or => " OR ",
    };
    let filters = rows.join(join);
    let Some(text) = query
        .search
        .as_deref()
        .filter(|text| !text.trim().is_empty())
    else {
        return Ok(filters);
    };
    let search = search_predicate(dialect, text.trim(), known, params)?;
    if filters.is_empty() {
        Ok(format!("({search})"))
    } else {
        Ok(format!("({filters}) AND ({search})"))
    }
}

fn search_predicate(
    dialect: Dialect,
    text: &str,
    known: &[String],
    params: &mut Vec<Option<String>>,
) -> Result<String, SelectError> {
    known
        .iter()
        .map(|column| {
            let filter = Filter {
                column: column.clone(),
                op: FilterOp::Contains,
                value: text.to_owned(),
                second: String::new(),
            };
            predicate::predicate(dialect, &filter, std::slice::from_ref(column), params)
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|rows| rows.join(" OR "))
}

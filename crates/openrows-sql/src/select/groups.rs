// SPDX-License-Identifier: Apache-2.0
//! Parenthesised filter groups share one bind-parameter sequence.
use super::{Combinator, SelectError, TableQuery, predicate};
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

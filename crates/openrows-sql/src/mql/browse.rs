// SPDX-License-Identifier: Apache-2.0
//! `MongoDB` collection pages through the shared table browser.
use crate::select::{
    BoundSql, Combinator, Filter, FilterOp, Page, SelectError, SortDirection, TableQuery,
};
use serde_json::{Value, json};
use std::fmt::Write;

pub(crate) fn page(query: &TableQuery, page: Option<Page>) -> Result<BoundSql, SelectError> {
    if query
        .sort
        .as_ref()
        .is_some_and(|sort| sort.column == "__document")
    {
        return Err(SelectError::Unsupported(
            "Use the MQL editor to sort document fields",
        ));
    }
    let filters = query
        .filters
        .iter()
        .map(filter)
        .collect::<Result<Vec<_>, _>>()?;
    let mut predicates = filters;
    for group in &query.groups {
        predicates.push(combine(
            group.combinator,
            group
                .filters
                .iter()
                .map(filter)
                .collect::<Result<Vec<_>, _>>()?,
        ));
    }
    let filter = combine(query.combinator, predicates);
    let collection = json!(query.table.name);
    let mut sql = if let Some(page) = page {
        format!(
            "db.getCollection({collection}).find({filter}).skip({}).limit({})",
            page.offset, page.limit
        )
    } else {
        format!("db.getCollection({collection}).countDocuments({filter})")
    };
    if let Some(sort) = query.sort.as_ref().filter(|_| page.is_some()) {
        let direction = if sort.direction == SortDirection::Ascending {
            1
        } else {
            -1
        };
        let _ = write!(sql, ".sort({})", json!({&sort.column:direction}));
    }
    if page.is_some() {
        sql.push_str(".toArray()");
    }
    Ok(BoundSql {
        sql,
        params: Vec::new(),
    })
}
fn combine(combinator: Combinator, filters: Vec<Value>) -> Value {
    if filters.is_empty() {
        return json!({});
    }
    let key = if combinator == Combinator::And {
        "$and"
    } else {
        "$or"
    };
    let mut object = serde_json::Map::new();
    object.insert(key.into(), Value::Array(filters));
    Value::Object(object)
}
fn filter(filter: &Filter) -> Result<Value, SelectError> {
    if filter.column == "__document" {
        return Err(SelectError::Unsupported(
            "Use the MQL editor to filter document fields",
        ));
    }
    let value: Value = serde_json::from_str(&filter.value).unwrap_or_else(|_| json!(filter.value));
    let operator = match filter.op {
        FilterOp::Equal => "$eq",
        FilterOp::NotEqual => "$ne",
        FilterOp::Less => "$lt",
        FilterOp::Greater => "$gt",
        FilterOp::LessOrEqual => "$lte",
        FilterOp::GreaterOrEqual => "$gte",
        FilterOp::IsNull => return Ok(json!({&filter.column: null})),
        FilterOp::IsNotNull => return Ok(json!({&filter.column: {"$ne":null}})),
        _ => {
            return Err(SelectError::Unsupported(
                "Use the MQL editor for this MongoDB filter",
            ));
        }
    };
    Ok(json!({&filter.column: {operator: value}}))
}

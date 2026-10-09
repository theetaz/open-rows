// SPDX-License-Identifier: Apache-2.0
//! `MySQL` `EXPLAIN FORMAT=JSON` and `MariaDB` `EXPLAIN`/`ANALYZE FORMAT=JSON`.
//!
//! Both nest operations (`ordering_operation`, `filesort`, …) around
//! `nested_loop` arrays of `table` objects; subqueries sit in arrays of
//! containers holding a `query_block`. Costs are each operator's own
//! (`read_cost` + `eval_cost`, `sort_cost`, `MariaDB`'s `cost`) except the
//! query block's total, so inclusive costs are summed here. `MariaDB`'s
//! `ANALYZE` adds `r_loops`, `r_rows` and total times (`r_*_time_ms`).
use crate::ParseError;
use crate::analysis::inclusive_cost;
use crate::json::{Object, field_number, field_text, text};
use crate::model::{Detail, NodeData, Plan, PlanNode};
use serde_json::Value;

/// Object keys that are plan operators, with their display names.
const OPERATORS: &[(&str, &str)] = &[
    ("query_block", "Query block"),
    ("table", "Table"),
    ("ordering_operation", "Order"),
    ("grouping_operation", "Group"),
    ("duplicates_removal", "Distinct"),
    ("windowing", "Window"),
    ("filesort", "Sort"),
    ("read_sorted_file", "Read sorted file"),
    ("temporary_table", "Temporary table"),
    ("block-nl-join", "Block nested loop"),
    ("materialized_from_subquery", "Materialize"),
    ("union_result", "Union"),
    ("buffer_result", "Buffer result"),
];

/// Keys mapped onto model fields rather than shown as details.
const CONSUMED: &[&str] = &[
    "select_id",
    "table_name",
    "key",
    "rows_examined_per_scan",
    "rows",
    "r_rows",
    "r_loops",
    "cost",
    "cost_info",
    "r_total_time_ms",
    "r_table_time_ms",
    "r_other_time_ms",
];

/// Keys holding conditions, with their labels.
const CONDITIONS: &[(&str, &str)] = &[
    ("attached_condition", "Condition"),
    ("index_condition", "Index condition"),
    ("having_condition", "Having"),
    ("sort_key", "Sort key"),
];

/// Parse one `EXPLAIN FORMAT=JSON` or `ANALYZE FORMAT=JSON` document.
pub(crate) fn parse(raw: &str) -> Result<Plan, ParseError> {
    let value: Value = serde_json::from_str(raw)?;
    let top = value
        .as_object()
        .ok_or(ParseError::Missing("plan document"))?;
    let block = top
        .get("query_block")
        .and_then(Value::as_object)
        .ok_or(ParseError::Missing("query_block"))?;
    let root = operator("query_block", block);
    let planning = top
        .get("query_optimization")
        .and_then(Value::as_object)
        .and_then(|optimisation| field_number(optimisation, "r_total_time_ms"));
    Ok(Plan {
        analyzed: has_actuals(&root),
        execution_ms: field_number(block, "r_total_time_ms"),
        root,
        planning_ms: planning,
        summary: Vec::new(),
    })
}

fn has_actuals(node: &PlanNode) -> bool {
    node.data.loops.is_some() || node.children.iter().any(has_actuals)
}

fn operator(kind: &str, object: &Object) -> PlanNode {
    let children = children(object);
    let mut data = NodeData::new(name(kind, object));
    data.relation = field_text(object, "table_name");
    data.index = field_text(object, "key");
    data.estimated_rows =
        field_number(object, "rows_examined_per_scan").or_else(|| field_number(object, "rows"));
    actuals(object, &mut data);
    let access = field_text(object, "access_type").unwrap_or_default();
    data.full_scan = access == "ALL" || access == "hash_ALL";
    if data.full_scan {
        let rows = data.actual_rows.or(data.estimated_rows);
        let loops = data.loops.or_else(|| field_number(object, "loops"));
        data.examined_rows = rows.map(|rows| rows * loops.unwrap_or(1.0));
    }
    for (key, label) in CONDITIONS {
        if let Some(value) = field_text(object, key) {
            data.conditions.push(Detail::new(*label, value));
        }
    }
    data.details = details(object);
    let mut node = PlanNode { data, children };
    node.data.total_cost = cost(kind, object, &node);
    node
}

/// `Query block #2`, `Full table scan`, `Sort`, `Update`…
fn name(kind: &str, object: &Object) -> String {
    if kind == "table" {
        return table_name(object);
    }
    if kind == "ordering_operation" && object.get("using_filesort") == Some(&Value::Bool(true)) {
        return "Sort".to_owned();
    }
    let base = OPERATORS
        .iter()
        .find(|(key, _)| *key == kind)
        .map_or(kind, |(_, label)| label);
    match field_text(object, "select_id") {
        Some(id) if kind == "query_block" => format!("{base} #{id}"),
        _ => base.to_owned(),
    }
}

/// A table access named by what it does: a write, or how rows are found.
fn table_name(object: &Object) -> String {
    for write in ["insert", "update", "delete"] {
        if object.get(write) == Some(&Value::Bool(true)) {
            let mut name = write.to_owned();
            name[..1].make_ascii_uppercase();
            return name;
        }
    }
    let access = field_text(object, "access_type").unwrap_or_default();
    match access.as_str() {
        "ALL" | "hash_ALL" => "Full table scan",
        "index" => "Full index scan",
        "range" | "hash_range" => "Index range scan",
        "ref" | "hash_ref" => "Index lookup",
        "eq_ref" | "hash_eq_ref" => "Unique index lookup",
        "const" | "system" => "Constant lookup",
        "ref_or_null" => "Index lookup or null",
        "index_merge" => "Index merge",
        "fulltext" => "Full-text search",
        "unique_subquery" | "index_subquery" => "Subquery index lookup",
        _ => "Table",
    }
    .to_owned()
}

/// `MariaDB` `ANALYZE` actuals: total times become per-loop averages.
fn actuals(object: &Object, data: &mut NodeData) {
    data.loops = field_number(object, "r_loops");
    data.actual_rows = field_number(object, "r_rows");
    let total = field_number(object, "r_total_time_ms").or_else(|| {
        let table = field_number(object, "r_table_time_ms")?;
        Some(table + field_number(object, "r_other_time_ms").unwrap_or(0.0))
    });
    let loops = data.loops.filter(|loops| *loops > 0.0).unwrap_or(1.0);
    data.actual_total_ms = total.map(|total| total / loops);
}

/// The query block's total, or the operator's own cost plus its inputs'.
fn cost(kind: &str, object: &Object, node: &PlanNode) -> Option<f64> {
    let info = object.get("cost_info").and_then(Value::as_object);
    if kind == "query_block" {
        return info
            .and_then(|info| field_number(info, "query_cost"))
            .or_else(|| field_number(object, "cost"));
    }
    let own = info
        .and_then(|info| {
            let read = field_number(info, "read_cost")?;
            Some(read + field_number(info, "eval_cost").unwrap_or(0.0))
        })
        .or_else(|| info.and_then(|info| field_number(info, "sort_cost")))
        .or_else(|| field_number(object, "cost"))?;
    let inputs: f64 = node.children.iter().filter_map(inclusive_cost).sum();
    Some(own + inputs)
}

/// Child operators: operator keys, `nested_loop` arrays and any container
/// (subquery lists, specifications) holding operators.
fn children(object: &Object) -> Vec<PlanNode> {
    let mut children = Vec::new();
    for (key, value) in object {
        match value {
            Value::Object(inner) if is_operator(key) => children.push(operator(key, inner)),
            Value::Object(inner) => children.extend(self::children(inner)),
            Value::Array(items) if key == "nested_loop" => children.push(PlanNode {
                data: NodeData::new("Nested loop"),
                children: items
                    .iter()
                    .filter_map(Value::as_object)
                    .flat_map(self::children)
                    .collect(),
            }),
            Value::Array(items) => children.extend(
                items
                    .iter()
                    .filter_map(Value::as_object)
                    .flat_map(self::children),
            ),
            _ => {}
        }
    }
    children
}

fn is_operator(key: &str) -> bool {
    OPERATORS.iter().any(|(operator, _)| *operator == key)
}

/// Scalars, scalar lists, cost entries and other plain objects.
fn details(object: &Object) -> Vec<Detail> {
    let mut details = Vec::new();
    for (key, value) in object {
        let condition = CONDITIONS.iter().any(|(name, _)| name == key);
        if condition || is_operator(key) || key == "nested_loop" || nests_operators(value) {
            continue;
        }
        if key == "cost_info" {
            if let Value::Object(info) = value {
                details.extend(
                    info.iter()
                        .map(|(name, value)| Detail::new(name.as_str(), text(value))),
                );
            }
        } else if !CONSUMED.contains(&key.as_str()) {
            details.push(Detail::new(key.as_str(), text(value)));
        }
    }
    details
}

fn nests_operators(value: &Value) -> bool {
    match value {
        Value::Object(inner) => inner
            .iter()
            .any(|(key, value)| is_operator(key) || nests_operators(value)),
        Value::Array(items) => items.iter().any(nests_operators),
        _ => false,
    }
}

#[cfg(test)]
#[path = "mysql_json_tests.rs"]
mod tests;

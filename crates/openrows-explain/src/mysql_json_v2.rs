// SPDX-License-Identifier: Apache-2.0
//! `MySQL`'s version 2 `EXPLAIN FORMAT=JSON` (`explain_json_format_version=2`,
//! the default on 9.7): a `query_plan` tree whose nodes name their `operation`
//! and feed from `inputs`, with inclusive costs like the tree format.
use crate::json::{Object, field_number, field_text};
use crate::model::{Detail, NodeData, PlanNode};
use serde_json::Value;

/// Keys mapped onto model fields or shown as conditions.
const CONSUMED: &[&str] = &[
    "operation",
    "inputs",
    "table_name",
    "index_name",
    "estimated_rows",
    "estimated_total_cost",
    "condition",
    "lookup_condition",
];

/// The operator tree below `query_plan`.
pub(crate) fn operator(object: &Object) -> PlanNode {
    let mut data = NodeData::new(field_text(object, "operation").unwrap_or_default());
    data.relation = field_text(object, "table_name");
    data.index = field_text(object, "index_name");
    data.estimated_rows = field_number(object, "estimated_rows");
    data.total_cost = field_number(object, "estimated_total_cost");
    data.full_scan = field_text(object, "access_type").as_deref() == Some("table");
    if data.full_scan {
        data.examined_rows = data.estimated_rows;
    }
    for (key, label) in [("condition", "Condition"), ("lookup_condition", "Lookup")] {
        if let Some(value) = field_text(object, key) {
            data.conditions.push(Detail::new(label, value));
        }
    }
    data.details = object
        .iter()
        .filter(|(key, _)| !CONSUMED.contains(&key.as_str()))
        .map(|(key, value)| Detail::new(key.clone(), detail_text(value)))
        .collect();
    let children = object
        .get("inputs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
        .map(operator)
        .collect();
    PlanNode { data, children }
}

fn detail_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

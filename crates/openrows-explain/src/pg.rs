// SPDX-License-Identifier: Apache-2.0
//! `PostgreSQL` `EXPLAIN (FORMAT JSON)` into the plan model.
use crate::ParseError;
use crate::json::{Object, field_number, field_text, text};
use crate::model::{Buffers, Detail, NodeData, Plan, PlanNode};
use serde_json::Value;

/// Keys that hold conditions, shown under Conditions in this order.
const CONDITIONS: &[&str] = &[
    "Index Cond",
    "Recheck Cond",
    "TID Cond",
    "Hash Cond",
    "Merge Cond",
    "Join Filter",
    "Filter",
    "One-Time Filter",
    "Order By",
];

/// Keys mapped onto model fields rather than shown as details.
const CONSUMED: &[&str] = &[
    "Plans",
    "Node Type",
    "Relation Name",
    "Schema",
    "Index Name",
    "Plan Rows",
    "Actual Rows",
    "Actual Loops",
    "Startup Cost",
    "Total Cost",
    "Actual Startup Time",
    "Actual Total Time",
    "Shared Hit Blocks",
    "Shared Read Blocks",
    "Shared Dirtied Blocks",
    "Shared Written Blocks",
    "Temp Read Blocks",
    "Temp Written Blocks",
];

/// Parse the single JSON document `EXPLAIN (FORMAT JSON)` returns.
pub(crate) fn parse(raw: &str) -> Result<Plan, ParseError> {
    let value: Value = serde_json::from_str(raw)?;
    let top = match &value {
        Value::Array(items) => items.first(),
        other => Some(other),
    }
    .and_then(Value::as_object)
    .ok_or(ParseError::Missing("plan document"))?;
    let root = top
        .get("Plan")
        .and_then(Value::as_object)
        .map(node)
        .ok_or(ParseError::Missing("Plan"))?;
    Ok(Plan {
        analyzed: root.data.actual_total_ms.is_some(),
        root,
        planning_ms: field_number(top, "Planning Time"),
        execution_ms: field_number(top, "Execution Time"),
        summary: summary(top),
    })
}

/// Settings, triggers, JIT and planning buffers of the whole statement.
fn summary(top: &Object) -> Vec<Detail> {
    let mut summary = Vec::new();
    if let Some(Value::Object(settings)) = top.get("Settings") {
        let pairs = settings
            .iter()
            .map(|(name, value)| format!("{name} = {}", text(value)));
        summary.push(Detail::new(
            "Settings",
            pairs.collect::<Vec<_>>().join(", "),
        ));
    }
    if let Some(Value::Array(triggers)) = top.get("Triggers") {
        for trigger in triggers.iter().filter_map(Value::as_object) {
            let name = field_text(trigger, "Trigger Name").unwrap_or_default();
            let time = field_text(trigger, "Time").unwrap_or_default();
            let calls = field_text(trigger, "Calls").unwrap_or_default();
            summary.push(Detail::new(
                format!("Trigger {name}"),
                format!("{time} ms, {calls} calls"),
            ));
        }
    }
    for key in ["JIT", "Planning"] {
        if let Some(value) = top.get(key) {
            summary.push(Detail::new(key, text(value)));
        }
    }
    summary
}

fn node(object: &Object) -> PlanNode {
    let children: Vec<PlanNode> = match object.get("Plans") {
        Some(Value::Array(plans)) => plans
            .iter()
            .filter_map(Value::as_object)
            .map(node)
            .collect(),
        _ => Vec::new(),
    };
    let buffers = buffers(object);
    let mut data = NodeData {
        node_type: node_name(object),
        relation: relation(object),
        index: field_text(object, "Index Name"),
        estimated_rows: field_number(object, "Plan Rows"),
        actual_rows: field_number(object, "Actual Rows"),
        loops: field_number(object, "Actual Loops"),
        startup_cost: field_number(object, "Startup Cost"),
        total_cost: field_number(object, "Total Cost"),
        actual_startup_ms: field_number(object, "Actual Startup Time"),
        actual_total_ms: field_number(object, "Actual Total Time"),
        full_scan: field_text(object, "Node Type").as_deref() == Some("Seq Scan"),
        spill: spill(object, buffers, &children),
        buffers,
        ..NodeData::default()
    };
    if data.full_scan {
        data.examined_rows = examined(object, &data);
    }
    for key in CONDITIONS {
        if let Some(value) = field_text(object, key) {
            data.conditions.push(Detail::new(*key, value));
        }
    }
    data.details = object
        .iter()
        .filter(|(key, _)| !CONSUMED.contains(&key.as_str()) && !CONDITIONS.contains(&key.as_str()))
        .map(|(key, value)| Detail::new(key.as_str(), text(value)))
        .collect();
    PlanNode { data, children }
}

/// The operator as `EXPLAIN`'s text format names it.
fn node_name(object: &Object) -> String {
    let base = field_text(object, "Node Type").unwrap_or_else(|| "Unknown".to_owned());
    let named = match base.as_str() {
        "ModifyTable" => field_text(object, "Operation").unwrap_or(base),
        "Aggregate" => match field_text(object, "Strategy").as_deref() {
            Some("Sorted") => "GroupAggregate".to_owned(),
            Some("Hashed") => "HashAggregate".to_owned(),
            Some("Mixed") => "MixedAggregate".to_owned(),
            _ => base,
        },
        "Hash Join" | "Merge Join" | "Nested Loop" => join_name(object, base),
        _ => base,
    };
    if object.get("Parallel Aware") == Some(&Value::Bool(true)) {
        format!("Parallel {named}")
    } else {
        named
    }
}

/// `Hash Left Join`, `Nested Loop Anti Join`; inner joins keep the plain name.
fn join_name(object: &Object, base: String) -> String {
    match field_text(object, "Join Type") {
        Some(kind) if kind != "Inner" => match base.strip_suffix(" Join") {
            Some(method) => format!("{method} {kind} Join"),
            None => format!("{base} {kind} Join"),
        },
        _ => base,
    }
}

/// `schema.table`, a CTE or a function the node reads.
fn relation(object: &Object) -> Option<String> {
    let name = field_text(object, "Relation Name")
        .or_else(|| field_text(object, "CTE Name"))
        .or_else(|| field_text(object, "Function Name"))?;
    Some(match field_text(object, "Schema") {
        Some(schema) => format!("{schema}.{name}"),
        None => name,
    })
}

fn buffers(object: &Object) -> Option<Buffers> {
    let block = |key: &str| field_number(object, key);
    block("Shared Hit Blocks")?;
    Some(Buffers {
        shared_hit: block("Shared Hit Blocks").unwrap_or(0.0),
        shared_read: block("Shared Read Blocks").unwrap_or(0.0),
        shared_dirtied: block("Shared Dirtied Blocks").unwrap_or(0.0),
        shared_written: block("Shared Written Blocks").unwrap_or(0.0),
        temp_read: block("Temp Read Blocks").unwrap_or(0.0),
        temp_written: block("Temp Written Blocks").unwrap_or(0.0),
    })
}

/// Why a sort, hash or the node itself went to disk. Buffer counts include
/// the children's, so temporary writes are judged on the node's own share.
fn spill(object: &Object, buffers: Option<Buffers>, children: &[PlanNode]) -> Option<String> {
    let sort_method = field_text(object, "Sort Method").unwrap_or_default();
    if sort_method.contains("external")
        || field_text(object, "Sort Space Type").as_deref() == Some("Disk")
    {
        let space = field_text(object, "Sort Space Used").unwrap_or_default();
        return Some(format!("Sort spilled to disk ({sort_method}, {space} kB)"));
    }
    let batches = field_number(object, "Hash Batches")
        .or_else(|| field_number(object, "HashAgg Batches"))
        .unwrap_or(1.0);
    if batches > 1.0 {
        return Some(format!("Hash split into {batches} batches on disk"));
    }
    let children_written: f64 = children
        .iter()
        .filter_map(|child| child.data.buffers)
        .map(|child| child.temp_written)
        .sum();
    let own = buffers.map_or(0.0, |buffers| buffers.temp_written) - children_written;
    (own > 0.0).then(|| format!("Wrote {own} temporary blocks"))
}

/// Rows a sequential scan read: returned plus filtered out, over all loops;
/// the estimate when the plan was not analysed.
fn examined(object: &Object, data: &NodeData) -> Option<f64> {
    let Some(actual) = data.actual_rows else {
        return data.estimated_rows;
    };
    let removed = field_number(object, "Rows Removed by Filter").unwrap_or(0.0);
    Some((actual + removed) * data.loops.unwrap_or(1.0))
}

#[cfg(test)]
#[path = "pg_tests.rs"]
mod tests;

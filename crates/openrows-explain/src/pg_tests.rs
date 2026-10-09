// SPDX-License-Identifier: Apache-2.0
//! Parser tests on real `PostgreSQL` 17 output captured from a container.
use super::*;

type TestResult = Result<(), ParseError>;

fn types(node: &PlanNode, out: &mut Vec<String>) {
    out.push(node.data.node_type.clone());
    for child in &node.children {
        types(child, out);
    }
}

#[test]
fn an_analysed_join_keeps_estimates_actuals_and_conditions() -> TestResult {
    let plan = parse(include_str!("../fixtures/pg_analyze.json"))?;
    assert!(plan.analyzed);
    assert!(plan.planning_ms.is_some() && plan.execution_ms.is_some());
    let mut names = Vec::new();
    types(&plan.root, &mut names);
    assert_eq!(
        names,
        [
            "Sort",
            "GroupAggregate",
            "Nested Loop",
            "Seq Scan",
            "Bitmap Heap Scan",
            "Bitmap Index Scan"
        ]
    );
    let scan = &plan.root.children[0].children[0].children[0].data;
    assert_eq!(scan.relation.as_deref(), Some("public.customers"));
    assert_eq!(
        (scan.estimated_rows, scan.actual_rows),
        (Some(1.0), Some(200.0))
    );
    assert!(scan.full_scan);
    assert_eq!(scan.examined_rows, Some(2000.0));
    assert_eq!(scan.conditions[0].label, "Filter");
    assert!(scan.buffers.is_some());
    let index = &plan.root.children[0].children[0].children[1].children[0].data;
    assert_eq!(index.index.as_deref(), Some("orders_customer"));
    assert_eq!(index.loops, Some(200.0));
    assert!(plan.summary.iter().any(|detail| detail.label == "Settings"));
    Ok(())
}

#[test]
fn a_plain_plan_has_estimates_only() -> TestResult {
    let plan = parse(include_str!("../fixtures/pg_plan.json"))?;
    assert!(!plan.analyzed);
    assert_eq!(plan.root.data.node_type, "Limit");
    assert_eq!(plan.root.data.total_cost, Some(97.47));
    assert_eq!(plan.root.data.actual_rows, None);
    assert_eq!(plan.execution_ms, None);
    Ok(())
}

#[test]
fn modify_table_is_named_by_its_operation() -> TestResult {
    let plan = parse(include_str!("../fixtures/pg_update.json"))?;
    assert_eq!(plan.root.data.node_type, "Update");
    assert_eq!(plan.root.data.relation.as_deref(), Some("public.orders"));
    Ok(())
}

#[test]
fn external_sorts_and_batched_hashes_are_spills() -> TestResult {
    let sort = parse(include_str!("../fixtures/pg_sort_spill.json"))?;
    let reason = sort.root.data.spill.unwrap_or_default();
    assert!(
        reason.starts_with("Sort spilled to disk (external merge"),
        "{reason}"
    );
    let hash = parse(include_str!("../fixtures/pg_hash_spill.json"))?;
    assert_eq!(hash.root.data.spill, None, "only the join and hash wrote");
    let join = &hash.root.children[0];
    assert!(join.data.spill.is_some());
    assert_eq!(
        join.children[1].data.spill.as_deref(),
        Some("Hash split into 32 batches on disk")
    );
    Ok(())
}

#[test]
fn joins_aggregates_and_parallel_nodes_get_text_format_names() {
    let name = |value: Value| node_name(value.as_object().unwrap_or(&Object::new()));
    assert_eq!(
        name(serde_json::json!({"Node Type": "Hash Join", "Join Type": "Left"})),
        "Hash Left Join"
    );
    assert_eq!(
        name(serde_json::json!({"Node Type": "Nested Loop", "Join Type": "Anti"})),
        "Nested Loop Anti Join"
    );
    assert_eq!(
        name(serde_json::json!({"Node Type": "Aggregate", "Strategy": "Hashed"})),
        "HashAggregate"
    );
    assert_eq!(
        name(serde_json::json!({"Node Type": "Seq Scan", "Parallel Aware": true})),
        "Parallel Seq Scan"
    );
}

#[test]
fn malformed_output_is_an_error() {
    assert!(parse("not json").is_err());
    assert!(matches!(parse("[{}]"), Err(ParseError::Missing("Plan"))));
}

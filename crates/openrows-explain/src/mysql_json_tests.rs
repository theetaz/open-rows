// SPDX-License-Identifier: Apache-2.0
//! Parser tests on real `MySQL` 8.4 and `MariaDB` 11.4 output captured from
//! containers.
use super::*;

type TestResult = Result<(), ParseError>;

fn find<'a>(node: &'a PlanNode, name: &str) -> Option<&'a PlanNode> {
    if node.data.node_type == name {
        return Some(node);
    }
    node.children.iter().find_map(|child| find(child, name))
}

#[test]
fn a_mysql_join_nests_operations_around_the_nested_loop() -> TestResult {
    let plan = parse(include_str!("../fixtures/my_plan.json"))?;
    assert!(!plan.analyzed);
    let root = &plan.root;
    assert_eq!(root.data.node_type, "Query block #1");
    assert_eq!(root.data.total_cost, Some(1262.59));
    let order = &root.children[0];
    assert_eq!(order.data.node_type, "Order");
    let group = &order.children[0];
    assert_eq!(group.data.node_type, "Group");
    let nested = &group.children[0];
    assert_eq!(nested.data.node_type, "Nested loop");
    let [scan, lookup] = nested.children.as_slice() else {
        return Err(ParseError::Missing("two tables"));
    };
    assert_eq!(scan.data.node_type, "Full table scan");
    assert_eq!(scan.data.relation.as_deref(), Some("c"));
    assert!(scan.data.full_scan);
    assert_eq!(scan.data.examined_rows, Some(2000.0));
    assert_eq!(scan.data.total_cost, Some(201.5));
    assert_eq!(scan.data.conditions[0].label, "Condition");
    assert_eq!(lookup.data.node_type, "Index lookup");
    assert_eq!(lookup.data.index.as_deref(), Some("orders_customer"));
    assert_eq!(lookup.data.estimated_rows, Some(15.0));
    assert!(
        lookup
            .data
            .details
            .iter()
            .any(|detail| detail.label == "prefix_cost")
    );
    Ok(())
}

#[test]
fn unions_and_materialised_subqueries_become_children() -> TestResult {
    let plan = parse(include_str!("../fixtures/my_union.json"))?;
    let union = &plan.root.children[0];
    assert_eq!(union.data.node_type, "Union");
    assert_eq!(union.children.len(), 2, "one query block per specification");
    assert!(find(&plan.root, "Materialize").is_some());
    Ok(())
}

#[test]
fn a_mysql_update_is_named_as_a_write() -> TestResult {
    let plan = parse(include_str!("../fixtures/my_update.json"))?;
    let table = &plan.root.children[0].data;
    assert_eq!(table.node_type, "Update");
    assert_eq!(table.relation.as_deref(), Some("orders"));
    Ok(())
}

#[test]
fn mariadb_plans_use_rows_and_cost() -> TestResult {
    let plan = parse(include_str!("../fixtures/ma_plan.json"))?;
    assert!(!plan.analyzed);
    let nested = &plan.root.children[0];
    let scan = &nested.children[0].data;
    assert_eq!(
        (scan.estimated_rows, scan.total_cost),
        (Some(2000.0), Some(0.341_128_8))
    );
    assert_eq!(scan.examined_rows, Some(2000.0));
    Ok(())
}

#[test]
fn mariadb_analyze_reports_per_loop_rows_and_times() -> TestResult {
    let plan = parse(include_str!("../fixtures/ma_analyze.json"))?;
    assert!(plan.analyzed);
    assert!(plan.planning_ms.is_some());
    assert_eq!(plan.execution_ms, Some(3.941_246_166));
    let lookup = &plan.root.children[0].children[1].data;
    assert_eq!(
        (lookup.loops, lookup.actual_rows),
        (Some(200.0), Some(15.0))
    );
    let total = lookup.actual_total_ms.unwrap_or_default() * 200.0;
    assert!((total - (3.192_311_213 + 0.271_022_037)).abs() < 1e-9);
    Ok(())
}

#[test]
fn mariadb_filesort_keeps_its_sort_key_and_table() -> TestResult {
    let plan = parse(include_str!("../fixtures/ma_sort.json"))?;
    let sort = find(&plan.root, "Sort").ok_or(ParseError::Missing("Sort"))?;
    assert_eq!(
        sort.data.conditions[0].value,
        "orders.total, orders.created"
    );
    assert_eq!(sort.children[0].data.relation.as_deref(), Some("orders"));
    assert!(sort.children[0].data.full_scan);
    Ok(())
}

#[test]
fn documents_without_a_query_block_are_errors() {
    assert!(matches!(
        parse("{}"),
        Err(ParseError::Missing("query_block"))
    ));
    assert!(parse("[").is_err());
}

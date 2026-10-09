// SPDX-License-Identifier: Apache-2.0
use super::*;
use crate::ParseError;

fn node(name: &str, total_ms: f64, children: Vec<PlanNode>) -> PlanNode {
    PlanNode {
        data: NodeData {
            actual_total_ms: Some(total_ms),
            loops: Some(1.0),
            ..NodeData::new(name)
        },
        children,
    }
}

fn plan(root: PlanNode, analyzed: bool) -> Plan {
    Plan {
        root,
        analyzed,
        planning_ms: None,
        execution_ms: None,
        summary: Vec::new(),
    }
}

#[test]
fn exclusive_time_subtracts_the_inputs_and_shares_sum_to_one() {
    let tree = node(
        "Sort",
        10.0,
        vec![node(
            "Join",
            8.0,
            vec![node("Scan a", 5.0, vec![]), node("Scan b", 1.0, vec![])],
        )],
    );
    let analysed = analyse(plan(tree, true));
    assert_eq!(analysed.weight, Weight::Time);
    let exclusive: Vec<Option<f64>> = analysed
        .nodes
        .iter()
        .map(|node| node.exclusive_ms)
        .collect();
    assert_eq!(exclusive, [Some(2.0), Some(2.0), Some(5.0), Some(1.0)]);
    assert_eq!(analysed.nodes[2].parent, Some(1));
    assert_eq!(analysed.nodes[2].depth, 2);
    let total: f64 = analysed.nodes.iter().map(|node| node.share).sum();
    assert!((total - 1.0).abs() < 1e-9);
}

#[test]
fn loops_multiply_per_loop_time_and_overlap_never_goes_negative() {
    let mut inner = node("Index lookup", 0.5, vec![]);
    inner.data.loops = Some(100.0);
    let analysed = analyse(plan(
        node(
            "Nested loop",
            20.0,
            vec![inner, node("Init plan", 30.0, vec![])],
        ),
        true,
    ));
    assert_eq!(analysed.nodes[1].inclusive_ms, Some(50.0));
    assert_eq!(analysed.nodes[0].exclusive_ms, Some(0.0));
}

#[test]
fn only_the_top_three_operators_above_ten_percent_are_expensive() {
    let leaves = [40.0, 30.0, 20.0, 9.0, 1.0].map(|ms| node("Scan", ms, vec![]));
    let analysed = analyse(plan(node("Append", 100.0, leaves.to_vec()), true));
    let expensive: Vec<usize> = analysed
        .nodes
        .iter()
        .filter(|node| matches!(node.hotspots.first(), Some(Hotspot::Expensive { .. })))
        .map(|node| node.id)
        .collect();
    assert_eq!(expensive, [1, 2, 3]);
}

#[test]
fn plans_without_times_are_weighed_by_cost_through_costless_nodes() {
    let costed = |name: &str, cost: f64| {
        let mut data = NodeData::new(name);
        data.total_cost = Some(cost);
        PlanNode::leaf(data)
    };
    let loop_node = PlanNode {
        data: NodeData::new("Nested loop"),
        children: vec![costed("a", 30.0), costed("b", 60.0)],
    };
    let mut root = PlanNode {
        data: NodeData::new("Query block"),
        children: vec![loop_node],
    };
    root.data.total_cost = Some(100.0);
    let analysed = analyse(plan(root, false));
    assert_eq!(analysed.weight, Weight::Cost);
    assert_eq!(analysed.nodes[0].exclusive_cost, Some(10.0));
    assert_eq!(analysed.nodes[1].exclusive_cost, Some(0.0));
    assert!((analysed.nodes[3].share - 0.6).abs() < 1e-9);
}

#[test]
fn misestimates_need_a_tenfold_gap_in_either_direction() {
    let rows = |estimated: f64, actual: f64| {
        let mut data = NodeData::new("Scan");
        (data.estimated_rows, data.actual_rows, data.loops) =
            (Some(estimated), Some(actual), Some(1.0));
        local_hotspots(&data, true)
    };
    assert_eq!(
        rows(1.0, 200.0),
        [Hotspot::Misestimate {
            estimated: 1.0,
            actual: 200.0
        }]
    );
    assert_eq!(rows(5000.0, 3.0).len(), 1);
    assert!(rows(10.0, 100.0).is_empty(), "exactly 10× is not flagged");
    assert!(
        rows(0.0, 5.0).is_empty(),
        "small counts are compared from one row"
    );
}

#[test]
fn full_scans_of_small_tables_are_not_flagged_but_unknown_sizes_are() {
    let scan = |rows: Option<f64>| {
        let mut data = NodeData::new("Seq Scan");
        (data.full_scan, data.examined_rows) = (true, rows);
        local_hotspots(&data, false)
    };
    assert!(scan(Some(9_999.0)).is_empty());
    assert_eq!(
        scan(Some(60_000.0)),
        [Hotspot::FullScan {
            rows: Some(60_000.0)
        }]
    );
    assert_eq!(scan(None), [Hotspot::FullScan { rows: None }]);
}

#[test]
fn real_postgres_output_flags_the_misestimated_scan_and_the_spill() -> Result<(), ParseError> {
    let join = analyse(crate::pg::parse(include_str!(
        "../fixtures/pg_analyze.json"
    ))?);
    let scan = join
        .nodes
        .iter()
        .find(|node| node.data.node_type == "Seq Scan");
    assert!(
        scan.is_some_and(|scan| scan.hotspots.contains(&Hotspot::Misestimate {
            estimated: 1.0,
            actual: 200.0
        }))
    );
    let sort = analyse(crate::pg::parse(include_str!(
        "../fixtures/pg_sort_spill.json"
    ))?);
    assert!(
        sort.nodes[0]
            .hotspots
            .iter()
            .any(|hotspot| matches!(hotspot, Hotspot::Spill { .. }))
    );
    assert!(sort.nodes[1].hotspots.contains(&Hotspot::FullScan {
        rows: Some(60_000.0)
    }));
    Ok(())
}

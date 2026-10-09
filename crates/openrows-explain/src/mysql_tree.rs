// SPDX-License-Identifier: Apache-2.0
//! `MySQL` `EXPLAIN ANALYZE` tree text, one operator per line:
//!
//! ```text
//! -> Filter: (c.region = 'eu')  (cost=202 rows=200) (actual time=0.099..0.584 rows=200 loops=1)
//!     -> Table scan on c  (cost=202 rows=2000) (actual time=0.0875..0.419 rows=2000 loops=1)
//! ```
//!
//! Each level indents by four spaces. Costs are inclusive; actual time is
//! `first row..all rows` per loop and rows are per loop, as in `PostgreSQL`.
use crate::ParseError;
use crate::model::{Detail, NodeData, Plan, PlanNode};

/// Markers that start the metrics after an operator's description.
const METRICS: &[&str] = &["(cost=", "(actual time=", "(never executed)"];

/// Parse the tree text of one `EXPLAIN ANALYZE`.
pub(crate) fn parse(raw: &str) -> Result<Plan, ParseError> {
    let mut lines: Vec<(usize, NodeData)> = Vec::new();
    for line in raw.lines() {
        let body = line.trim_start();
        if let Some(operator) = body.strip_prefix("->") {
            let depth = (line.len() - body.len()) / 4;
            lines.push((depth, operator_line(operator.trim())));
        } else if let Some((_, last)) = lines.last_mut()
            && !body.is_empty()
        {
            last.details.push(Detail::new("More", body));
        }
    }
    if lines.is_empty() {
        return Err(ParseError::Missing("operator lines"));
    }
    let mut roots = build(&mut lines.into_iter().peekable(), 0);
    let root = if roots.len() == 1 {
        roots.remove(0)
    } else {
        PlanNode {
            data: NodeData::new("Query plan"),
            children: roots,
        }
    };
    Ok(Plan {
        analyzed: true,
        root,
        planning_ms: None,
        execution_ms: None,
        summary: Vec::new(),
    })
}

/// Nodes at `depth` and, recursively, their deeper followers.
fn build(
    lines: &mut std::iter::Peekable<impl Iterator<Item = (usize, NodeData)>>,
    depth: usize,
) -> Vec<PlanNode> {
    let mut nodes = Vec::new();
    while let Some((_, data)) = lines.next_if(|(level, _)| *level >= depth) {
        let children = match lines.peek() {
            Some((level, _)) if *level > depth => build(lines, depth + 1),
            _ => Vec::new(),
        };
        nodes.push(PlanNode { data, children });
    }
    nodes
}

fn operator_line(text: &str) -> NodeData {
    let start = METRICS
        .iter()
        .filter_map(|marker| text.find(marker))
        .min()
        .unwrap_or(text.len());
    let (description, metrics) = text.split_at(start);
    let mut data = describe(description.trim());
    for group in metrics.split(')') {
        apply_metrics(group.trim().trim_start_matches('('), &mut data);
    }
    if data.full_scan {
        data.examined_rows = match (data.actual_rows, data.loops) {
            (Some(rows), Some(loops)) => Some(rows * loops),
            _ => data.estimated_rows,
        };
    }
    data
}

/// Operator name, table, index and condition from the description.
fn describe(description: &str) -> NodeData {
    if let Some((name, rest)) = description.split_once(": ") {
        let mut data = NodeData::new(name);
        data.conditions.push(Detail::new(name, rest));
        return data;
    }
    if let Some((name, rest)) = description.split_once(" on ") {
        let mut data = NodeData::new(name);
        let (table, rest) = rest.split_once(' ').unwrap_or((rest, ""));
        data.relation = Some(table.to_owned());
        let rest = match rest.strip_prefix("using ") {
            Some(using) => {
                let (index, rest) = using.split_once(' ').unwrap_or((using, ""));
                data.index = Some(index.to_owned());
                rest
            }
            None => rest,
        };
        if !rest.is_empty() {
            data.conditions.push(Detail::new("Condition", rest));
        }
        data.full_scan = name == "Table scan" && !table.starts_with('<');
        return data;
    }
    match description.split_once(" (") {
        Some((name, rest)) => {
            let mut data = NodeData::new(name);
            data.conditions
                .push(Detail::new("Condition", format!("({rest}")));
            data
        }
        None => NodeData::new(description),
    }
}

/// `cost=1.5 rows=3`, `actual time=0.1..0.4 rows=3 loops=1` or `never executed`.
fn apply_metrics(group: &str, data: &mut NodeData) {
    if group == "never executed" {
        data.loops = Some(0.0);
        return;
    }
    let actual = group.starts_with("actual ");
    for pair in group.split_whitespace() {
        let Some((key, value)) = pair.split_once('=') else {
            continue;
        };
        let (first, last) = range(value);
        match (key, actual) {
            ("cost", false) => {
                (data.startup_cost, data.total_cost) =
                    (first.filter(|_| value.contains("..")), last);
            }
            ("rows", false) => data.estimated_rows = last,
            ("time", true) => (data.actual_startup_ms, data.actual_total_ms) = (first, last),
            ("rows", true) => data.actual_rows = last,
            ("loops", true) => data.loops = last,
            _ => {}
        }
    }
}

/// `a..b` as both ends; a single number as `(a, a)`.
fn range(value: &str) -> (Option<f64>, Option<f64>) {
    if let Some((first, last)) = value.split_once("..") {
        return (first.parse().ok(), last.parse().ok());
    }
    let number = value.parse().ok();
    (number, number)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_join_tree_keeps_levels_estimates_and_actuals() -> Result<(), ParseError> {
        let plan = parse(include_str!("../fixtures/my_analyze.txt"))?;
        assert!(plan.analyzed);
        let root = &plan.root;
        assert_eq!(root.data.node_type, "Group aggregate");
        assert_eq!(root.data.total_cost, Some(1566.0));
        let join = &root.children[0];
        assert_eq!(join.data.node_type, "Nested loop inner join");
        assert_eq!(join.children.len(), 2);
        let scan = &join.children[0].children[0].data;
        assert_eq!(scan.node_type, "Table scan");
        assert_eq!(scan.relation.as_deref(), Some("c"));
        assert!(scan.full_scan);
        assert_eq!(scan.examined_rows, Some(2000.0));
        let lookup = &join.children[1].data;
        assert_eq!(lookup.node_type, "Index lookup");
        assert_eq!(lookup.index.as_deref(), Some("orders_customer"));
        assert_eq!(lookup.conditions[0].value, "(customer_id=c.id)");
        assert_eq!(
            (lookup.estimated_rows, lookup.actual_rows),
            (Some(15.2), Some(15.0))
        );
        assert_eq!(lookup.loops, Some(200.0));
        assert_eq!(lookup.actual_total_ms, Some(0.0156));
        Ok(())
    }

    #[test]
    fn subqueries_without_metrics_nest_their_operators() -> Result<(), ParseError> {
        let plan = parse(include_str!("../fixtures/my_analyze_sub.txt"))?;
        let filter = &plan.root.children[0].children[0];
        assert_eq!(filter.data.node_type, "Filter");
        assert_eq!(filter.children.len(), 2);
        let select = &filter.children[1];
        assert_eq!(select.data.node_type, "Select #2");
        assert_eq!(select.data.total_cost, None);
        assert_eq!(
            select.children[0].children[0].data.relation.as_deref(),
            Some("orders")
        );
        Ok(())
    }

    #[test]
    fn never_executed_operators_have_no_actuals() -> Result<(), ParseError> {
        let plan = parse("-> Filter: (false)  (cost=0.1 rows=1) (never executed)\n")?;
        assert_eq!(plan.root.data.loops, Some(0.0));
        assert_eq!(plan.root.data.actual_rows, None);
        assert!(parse("no tree").is_err());
        Ok(())
    }

    #[test]
    fn several_top_level_operators_get_a_common_root() -> Result<(), ParseError> {
        let plan = parse("-> Rows fetched before execution\n-> Zero rows (no matching row)\n")?;
        assert_eq!(plan.root.data.node_type, "Query plan");
        assert_eq!(plan.root.children.len(), 2);
        Ok(())
    }
}

// SPDX-License-Identifier: Apache-2.0
//! `SQLite` `EXPLAIN QUERY PLAN` rows (`id`, `parent`, `notused`, `detail`)
//! into a tree. `SQLite` reports neither costs nor row counts, so only the
//! operator, table, index and search condition are known.
use crate::ParseError;
use crate::model::{Detail, NodeData, Plan, PlanNode};

/// One `EXPLAIN QUERY PLAN` row.
struct Row {
    id: i64,
    parent: i64,
    detail: String,
}

/// Parse the rows as text cells in column order.
pub(crate) fn parse(rows: &[Vec<Option<String>>]) -> Result<Plan, ParseError> {
    let rows = rows
        .iter()
        .map(|cells| row(cells))
        .collect::<Result<Vec<_>, _>>()?;
    if rows.is_empty() {
        return Err(ParseError::Missing("query plan rows"));
    }
    let mut roots = children_of(0, &rows);
    let root = if roots.len() == 1 {
        roots.remove(0)
    } else {
        PlanNode {
            data: NodeData::new("Query plan"),
            children: roots,
        }
    };
    Ok(Plan {
        root,
        analyzed: false,
        planning_ms: None,
        execution_ms: None,
        summary: Vec::new(),
    })
}

fn row(cells: &[Option<String>]) -> Result<Row, ParseError> {
    let number = |index: usize| {
        cells
            .get(index)
            .and_then(Option::as_deref)
            .and_then(|text| text.parse::<i64>().ok())
            .ok_or(ParseError::Missing("query plan id or parent"))
    };
    Ok(Row {
        id: number(0)?,
        parent: number(1)?,
        detail: cells.get(3).cloned().flatten().unwrap_or_default(),
    })
}

/// Rows whose parent is `parent`, in output order, with their subtrees.
/// Ids only grow, so a row is never its own ancestor.
fn children_of(parent: i64, rows: &[Row]) -> Vec<PlanNode> {
    rows.iter()
        .filter(|row| row.parent == parent && row.id > parent)
        .map(|row| PlanNode {
            data: describe(&row.detail),
            children: children_of(row.id, rows),
        })
        .collect()
}

/// `SCAN t`, `SEARCH t USING INDEX i (a=?)`, `USE TEMP B-TREE FOR ORDER BY`…
fn describe(detail: &str) -> NodeData {
    let (verb, rest) = detail.split_once(' ').unwrap_or((detail, ""));
    let mut data = match verb {
        "SCAN" | "SEARCH" => access(verb, rest),
        _ => NodeData::new(detail),
    };
    data.details.push(Detail::new("Detail", detail));
    data
}

/// A table access: full scan, covering-index scan or index search.
fn access(verb: &str, rest: &str) -> NodeData {
    // SQLite before 3.36 wrote `SCAN TABLE t`.
    let rest = rest.strip_prefix("TABLE ").unwrap_or(rest);
    let (table, using) = rest.split_once(' ').unwrap_or((rest, ""));
    let (index, condition) = index_of(using);
    let name = match (verb, &index) {
        ("SCAN", None) => "Full scan",
        ("SCAN", Some(_)) => "Index scan",
        _ => "Index search",
    };
    let mut data = NodeData::new(name);
    data.relation = Some(table.to_owned());
    data.full_scan = verb == "SCAN" && index.is_none() && table != "CONSTANT";
    data.index = index;
    if let Some(condition) = condition {
        data.conditions.push(Detail::new("Condition", condition));
    }
    data
}

/// The index of `USING [COVERING] INDEX name (cond)` or `USING INTEGER
/// PRIMARY KEY (cond)`, and the condition.
fn index_of(using: &str) -> (Option<String>, Option<String>) {
    let (head, condition) = match using.split_once(" (") {
        Some((head, condition)) => (head, Some(format!("({condition}"))),
        None => (using, None),
    };
    let index = ["USING COVERING INDEX ", "USING INDEX ", "USING "]
        .iter()
        .find_map(|prefix| head.strip_prefix(prefix))
        .map(str::to_owned);
    (index, condition)
}

/// `sqlite3`'s `.eqp` rendering of the rows, for the raw tab.
pub(crate) fn render(rows: &[Vec<Option<String>>]) -> String {
    let Ok(rows) = rows
        .iter()
        .map(|cells| row(cells))
        .collect::<Result<Vec<_>, _>>()
    else {
        return String::new();
    };
    let mut out = String::from("QUERY PLAN\n");
    render_children(0, &rows, "", &mut out);
    out
}

fn render_children(parent: i64, rows: &[Row], prefix: &str, out: &mut String) {
    let children: Vec<&Row> = rows
        .iter()
        .filter(|row| row.parent == parent && row.id > parent)
        .collect();
    for (index, row) in children.iter().enumerate() {
        let last = index + 1 == children.len();
        out.push_str(prefix);
        out.push_str(if last { "`--" } else { "|--" });
        out.push_str(&row.detail);
        out.push('\n');
        let deeper = format!("{prefix}{}", if last { "   " } else { "|  " });
        render_children(row.id, rows, &deeper, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(rows: &[(i64, i64, &str)]) -> Vec<Vec<Option<String>>> {
        rows.iter()
            .map(|(id, parent, detail)| {
                vec![
                    Some(id.to_string()),
                    Some(parent.to_string()),
                    Some("0".to_owned()),
                    Some((*detail).to_owned()),
                ]
            })
            .collect()
    }

    /// Captured from `SQLite` 3.51 for a join with a scalar subquery.
    fn join() -> Vec<Vec<Option<String>>> {
        rows(&[
            (9, 0, "SCAN c"),
            (
                13,
                0,
                "SEARCH o USING INDEX orders_customer (customer_id=?)",
            ),
            (22, 0, "SCALAR SUBQUERY 1"),
            (27, 22, "SCAN orders"),
            (66, 0, "USE TEMP B-TREE FOR ORDER BY"),
        ])
    }

    #[test]
    fn rows_become_a_tree_of_scans_and_searches() -> Result<(), ParseError> {
        let plan = parse(&join())?;
        assert!(!plan.analyzed);
        assert_eq!(plan.root.data.node_type, "Query plan");
        let [scan, search, subquery, sort] = plan.root.children.as_slice() else {
            return Err(ParseError::Missing("four top-level operators"));
        };
        assert_eq!(scan.data.node_type, "Full scan");
        assert!(scan.data.full_scan);
        assert_eq!(search.data.index.as_deref(), Some("orders_customer"));
        assert_eq!(search.data.conditions[0].value, "(customer_id=?)");
        assert_eq!(
            subquery.children[0].data.relation.as_deref(),
            Some("orders")
        );
        assert_eq!(sort.data.node_type, "USE TEMP B-TREE FOR ORDER BY");
        Ok(())
    }

    #[test]
    fn covering_and_primary_key_accesses_are_not_full_scans() -> Result<(), ParseError> {
        let plan = parse(&rows(&[
            (1, 0, "COMPOUND QUERY"),
            (2, 1, "LEFT-MOST SUBQUERY"),
            (
                5,
                2,
                "SEARCH orders USING INDEX orders_customer (customer_id=?)",
            ),
            (17, 1, "UNION ALL"),
            (19, 17, "SEARCH orders USING INTEGER PRIMARY KEY (rowid=?)"),
            (26, 17, "SCAN orders USING COVERING INDEX orders_customer"),
            (30, 0, "SCAN TABLE legacy"),
        ]))?;
        let union = &plan.root.children[0].children[1];
        assert_eq!(
            union.children[0].data.index.as_deref(),
            Some("INTEGER PRIMARY KEY")
        );
        let covering = &union.children[1].data;
        assert_eq!(
            (covering.node_type.as_str(), covering.full_scan),
            ("Index scan", false)
        );
        assert_eq!(
            plan.root.children[1].data.relation.as_deref(),
            Some("legacy")
        );
        Ok(())
    }

    #[test]
    fn render_matches_the_sqlite_shell() {
        let text = render(&join());
        assert_eq!(
            text,
            "QUERY PLAN\n|--SCAN c\n|--SEARCH o USING INDEX orders_customer (customer_id=?)\n\
             |--SCALAR SUBQUERY 1\n|  `--SCAN orders\n`--USE TEMP B-TREE FOR ORDER BY\n"
        );
    }

    #[test]
    fn rows_without_ids_are_an_error() {
        assert!(parse(&[vec![Some("x".to_owned())]]).is_err());
        assert!(parse(&[]).is_err());
    }
}

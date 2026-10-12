// SPDX-License-Identifier: Apache-2.0
//! Resolve simple plan keys conservatively; never copy conditions or values into advice.
use super::{NodeEvidence, PlanEvidence, TableEvidence};
use openrows_explain::{AnalysedNode, AnalysedPlan};

/// Project an explicitly obtained `PostgreSQL` plan without retaining raw conditions.
#[must_use]
pub fn project_plan(
    plan: &AnalysedPlan,
    scope: (&str, f64),
    tables: &[TableEvidence],
) -> PlanEvidence {
    let nodes: Vec<_> = plan
        .nodes
        .iter()
        .take(256)
        .map(|node| {
            let table = resolve(node.data.relation.as_deref(), tables);
            NodeEvidence {
                id: u32::try_from(node.id).unwrap_or(u32::MAX),
                table_id: table.map(|t| t.id.clone()),
                operator: node.data.node_type.clone(),
                estimated_rows: node.data.estimated_rows,
                actual_rows: node.data.actual_rows,
                loops: node.data.loops,
                filter_column: filter_column(node),
                join_columns: join_columns(node, plan),
                spill: plan.analyzed && node.data.spill.is_some(),
            }
        })
        .collect();
    PlanEvidence {
        statement_id: scope.0.into(),
        collected_ms: scope.1,
        measured: plan.analyzed,
        nodes,
    }
}

fn resolve<'a>(name: Option<&str>, tables: &'a [TableEvidence]) -> Option<&'a TableEvidence> {
    let name = name?;
    let mut matches = tables
        .iter()
        .filter(|t| name == format!("{}.{}", t.schema, t.name) || name == t.name);
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}
fn normalized_equality(value: &str) -> Option<(String, String)> {
    let normalized =
        openrows_sql::normalize_monitor_query(&format!("SELECT * FROM t WHERE {value}"))?;
    let expression = normalized
        .strip_prefix("SELECT * FROM t WHERE ")?
        .trim_matches(['(', ')', ' ']);
    let (left, right) = expression.split_once('=')?;
    if right.contains('=') || expression.contains(" AND ") || expression.contains(" OR ") {
        return None;
    }
    Some((left.trim().into(), right.trim().into()))
}
fn identifier(value: &str) -> Option<(&str, &str)> {
    let parts: Vec<_> = value.split('.').collect();
    if parts.is_empty()
        || parts.len() > 2
        || parts.iter().any(|part| {
            part.is_empty()
                || !part.chars().enumerate().all(|(i, c)| {
                    c == '_' || c.is_ascii_alphabetic() || (i > 0 && c.is_ascii_digit())
                })
        })
    {
        return None;
    }
    Some((
        if parts.len() == 2 { parts[0] } else { "" },
        parts[parts.len() - 1],
    ))
}
fn filter_column(node: &AnalysedNode) -> Option<String> {
    let condition = node.data.conditions.iter().find(|c| c.label == "Filter")?;
    let (left, right) = normalized_equality(&condition.value)?;
    // Casts on a literal are harmless; casts/functions on the key need expert review.
    if !right.starts_with('?') || right.chars().any(|c| c == ' ' || c == '(' || c == ')') {
        return None;
    }
    Some(identifier(&left)?.1.into())
}
fn join_columns(node: &AnalysedNode, plan: &AnalysedPlan) -> Vec<String> {
    let Some(alias) = node
        .data
        .details
        .iter()
        .find(|d| d.label == "Alias")
        .map(|d| d.value.as_str())
    else {
        return Vec::new();
    };
    let mut columns = Vec::new();
    let mut parent = node.parent;
    for _ in 0..16 {
        let Some(current) = parent.and_then(|id| plan.nodes.iter().find(|n| n.id == id)) else {
            break;
        };
        for condition in current.data.conditions.iter().filter(|condition| {
            matches!(
                condition.label.as_str(),
                "Hash Cond" | "Merge Cond" | "Join Filter"
            )
        }) {
            equality_keys(alias, &condition.value, &mut columns);
        }
        parent = current.parent;
    }
    columns.sort();
    columns.dedup();
    columns.truncate(8);
    columns
}
fn equality_keys(alias: &str, value: &str, columns: &mut Vec<String>) {
    let Some((left, right)) = normalized_equality(value) else {
        return;
    };
    let (Some((a, key)), Some((b, other))) = (identifier(&left), identifier(&right)) else {
        return;
    };
    if a == alias && !b.is_empty() && a != b {
        columns.push(key.into());
    }
    if b == alias && !a.is_empty() && a != b {
        columns.push(other.into());
    }
}

/// Conservative charge for bounded plan projections and their owned identifiers.
pub(crate) fn retained_bytes(plan: &PlanEvidence) -> usize {
    plan.statement_id.capacity()
        + 128
        + plan.nodes.capacity() * 256
        + plan
            .nodes
            .iter()
            .map(|node| {
                node.operator.capacity()
                    + node.table_id.as_ref().map_or(0, String::capacity)
                    + node.filter_column.as_ref().map_or(0, String::capacity)
                    + node
                        .join_columns
                        .iter()
                        .map(|c| c.capacity() + 32)
                        .sum::<usize>()
            })
            .sum::<usize>()
}

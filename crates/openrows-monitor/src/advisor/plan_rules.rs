// SPDX-License-Identifier: Apache-2.0
//! Index candidates and row-estimate/spill rules consume explicit plan evidence.
use super::{
    AdvisorInput, CATALOG_MAX_AGE_MS, NodeEvidence, PlanEvidence, Suggestion, TableEvidence,
    catalog_fresh, finite, plan_fresh, qualified, quote, suggestion,
};

/// Evaluate explicit plan nodes without converting estimates into measured outcomes.
pub(super) fn evaluate(input: &AdvisorInput) -> Vec<Suggestion> {
    let mut out = Vec::new();
    for plan in input.plans.iter().filter(|p| plan_fresh(p, input.now_ms)) {
        for node in &plan.nodes {
            let table = node
                .table_id
                .as_ref()
                .and_then(|id| input.tables.iter().find(|t| t.id == *id));
            if catalog_fresh(input)
                && let Some(table) = table
            {
                out.extend(scan(input, (plan, node), table));
                out.extend(join(input, (plan, node), table));
                out.extend(misestimate((plan, node), table));
            }
            if plan.measured && node.spill {
                out.push(spill(plan, node));
            }
        }
    }
    out
}
fn no_index(input: &AdvisorInput, table: &TableEvidence, column: &str) -> bool {
    input
        .indexes_coverage
        .fresh(input.now_ms, CATALOG_MAX_AGE_MS)
        && !input.indexes.iter().any(|i| {
            i.table_id == table.id && i.usable && i.keys.first().is_some_and(|key| key == column)
        })
}
fn scan(
    input: &AdvisorInput,
    (plan, node): (&PlanEvidence, &NodeEvidence),
    table: &TableEvidence,
) -> Option<Suggestion> {
    let (rows, estimated, column) = (
        finite(table.estimated_rows)?,
        finite(node.estimated_rows)?,
        node.filter_column.as_deref()?,
    );
    if !matches!(node.operator.as_str(), "Seq Scan" | "Parallel Seq Scan")
        || rows < 100_000.0
        || estimated > rows * 0.01
        || !no_index(input, table, column)
    {
        return None;
    }
    let mut card = index_card("pg.selective-scan", (plan, node), (table, column));
    card.evidence = format!(
        "Plan node {} {} on {}: {estimated:.0} estimated output / {rows:.0} estimated table rows ≤ 1%; table size {} bytes; no usable leading btree key {column} in complete fresh index snapshot.",
        node.id,
        node.operator,
        qualified(table),
        table
            .size_bytes
            .map_or_else(|| "unknown".into(), |v| format!("{v:.0}"))
    );
    Some(card)
}
fn join(
    input: &AdvisorInput,
    (plan, node): (&PlanEvidence, &NodeEvidence),
    table: &TableEvidence,
) -> Option<Suggestion> {
    if finite(table.estimated_rows)? < 100_000.0 || !node.operator.contains("Seq Scan") {
        return None;
    }
    let column = node
        .join_columns
        .iter()
        .find(|column| no_index(input, table, column))?;
    let mut card = index_card("pg.join-key", (plan, node), (table, column));
    card.confidence = "low".into();
    card.evidence = format!(
        "Plan node {} {} on {} has equality join key {column}; {:.0} estimated table rows and no usable leading index. A hash join/full scan can still be optimal.",
        node.id,
        node.operator,
        qualified(table),
        table.estimated_rows?
    );
    Some(card)
}
fn index_card(
    rule: &str,
    (plan, node): (&PlanEvidence, &NodeEvidence),
    (table, column): (&TableEvidence, &str),
) -> Suggestion {
    let identity = format!("{}:{}:{column}", plan.statement_id, node.id);
    let mut card = suggestion(rule, &identity, String::new(), plan.collected_ms);
    card.sql = Some(format!(
        "CREATE INDEX CONCURRENTLY ON {} ({});",
        qualified(table),
        quote(column)
    ));
    card.statement_id = Some(plan.statement_id.clone());
    card.evidence_link = format!("plan:{}", node.id);
    card.impact = "Potentially reduce scanned rows; compare plans before and after".into();
    card.trade_offs = "New indexes add storage and write amplification. CREATE INDEX CONCURRENTLY reduces write blocking but uses extra I/O, needs locks and cannot run in a transaction. Review operators/types, selectivity and production load; estimates are not measured improvement.".into();
    card
}
fn misestimate(
    (plan, node): (&PlanEvidence, &NodeEvidence),
    table: &TableEvidence,
) -> Option<Suggestion> {
    let (estimated, actual) = (finite(node.estimated_rows)?, finite(node.actual_rows)?);
    if !plan.measured
        || finite(node.loops)? == 0.0
        || (estimated <= 0.0 && actual <= 0.0)
        || (estimated.min(actual) > 0.0 && estimated.max(actual) / estimated.min(actual) < 10.0)
    {
        return None;
    }
    let mut card = suggestion(
        "pg.estimate-error",
        &format!("{}:{}", plan.statement_id, node.id),
        format!(
            "Measured plan node {} on {}: {estimated:.0} estimated vs {actual:.0} actual rows per loop (≥ 10× mismatch, or one side zero).",
            node.id,
            qualified(table)
        ),
        plan.collected_ms,
    );
    card.sql = Some(format!("ANALYZE {};", qualified(table)));
    card.statement_id = Some(plan.statement_id.clone());
    card.evidence_link = format!("plan:{}", node.id);
    card.impact = "Investigate skew/correlation and refresh planner statistics".into();
    card.trade_offs = "ANALYZE uses I/O/CPU and may not fix correlated predicates. Actuals apply only to this explicitly measured execution and its parameters.".into();
    Some(card)
}
fn spill(plan: &PlanEvidence, node: &NodeEvidence) -> Suggestion {
    let mut card = suggestion(
        "pg.plan-spill",
        &format!("{}:{}", plan.statement_id, node.id),
        format!(
            "Measured plan node {} ({}) reported disk spill.",
            node.id, node.operator
        ),
        plan.collected_ms,
    );
    card.confidence = "high".into();
    card.statement_id = Some(plan.statement_id.clone());
    card.evidence_link = format!("plan:{}", node.id);
    spill_notes(&mut card);
    card
}
/// Explain spill trade-offs without generating memory-setting SQL.
pub(super) fn spill_notes(card: &mut Suggestion) {
    card.impact =
        "Reduce temporary I/O after reviewing cardinality, projection and ordering indexes".into();
    card.trade_offs = "Review an ordering index or narrower query first. Consider session work_mem only after accounting for concurrent sorts/hashes and memory pressure. No SET or server-setting SQL is generated.".into();
}

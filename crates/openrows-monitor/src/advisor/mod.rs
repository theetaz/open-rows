// SPDX-License-Identifier: Apache-2.0
//! Versioned `PostgreSQL` proposals; this module has no database execution capability.
pub(crate) mod catalog;
mod lock_rules;
mod maintenance;
mod model;
pub(crate) mod plan;
mod plan_rules;
mod statement_rules;
pub use model::*;
pub use plan::project_plan;

/// Evaluate valid evidence, sorted by impact then stable identity, capped at 64 proposals.
#[must_use]
pub fn advise(input: &AdvisorInput) -> Vec<Suggestion> {
    let mut suggestions = maintenance::evaluate(input);
    suggestions.extend(plan_rules::evaluate(input));
    suggestions.extend(statement_rules::evaluate(input));
    suggestions.extend(lock_rules::evaluate(input));
    suggestions.sort_by(|a, b| {
        priority(&a.rule_id)
            .cmp(&priority(&b.rule_id))
            .then(a.id.cmp(&b.id))
    });
    suggestions.dedup_by(|a, b| a.id == b.id);
    suggestions.truncate(64);
    suggestions
}

/// Quote an exact catalog identifier for a proposal, including embedded quotes.
pub(super) fn quote(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}
/// Fully qualify a relation so proposals do not depend on the editor search path.
pub(super) fn qualified(table: &TableEvidence) -> String {
    format!("{}.{}", quote(&table.schema), quote(&table.name))
}
/// Create a versioned evidence card with conservative shared defaults.
pub(super) fn suggestion(
    rule: &str,
    identity: &str,
    evidence: String,
    collected_ms: f64,
) -> Suggestion {
    Suggestion {
        id: format!("{rule}:{identity}"), rule_id: rule.into(), version: RULE_VERSION,
        confidence: "medium".into(), impact: "Review expected workload benefit".into(),
        trade_offs: "Review locks, I/O and workload impact before running; estimates do not prove improvement.".into(),
        evidence, evidence_link: String::new(), sql: None, statement_id: None, collected_ms,
    }
}

/// Preserve unknown evidence and reject negative or non-finite metrics.
pub(super) fn finite(value: Option<f64>) -> Option<f64> {
    value.filter(|v| v.is_finite() && *v >= 0.0)
}
/// Require complete table metadata within the catalog freshness budget.
pub(super) fn catalog_fresh(input: &AdvisorInput) -> bool {
    input
        .tables_coverage
        .fresh(input.now_ms, CATALOG_MAX_AGE_MS)
}
/// Reject future or expired explicit plan evidence.
pub(super) fn plan_fresh(plan: &PlanEvidence, now: f64) -> bool {
    plan.collected_ms.is_finite()
        && now >= plan.collected_ms
        && now - plan.collected_ms <= PLAN_MAX_AGE_MS
}

fn priority(rule: &str) -> u32 {
    match rule {
        "pg.plan-spill" | "pg.statement-spill" | "pg.lock-hotspot" => 0,
        "pg.selective-scan" | "pg.estimate-error" => 1,
        "pg.stale-statistics" | "pg.vacuum" | "pg.join-key" => 2,
        "pg.duplicate-index" | "pg.overlapping-index" => 3,
        _ => 4,
    }
}

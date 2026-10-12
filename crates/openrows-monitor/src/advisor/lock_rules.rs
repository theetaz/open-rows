// SPDX-License-Identifier: Apache-2.0
//! A repeated actual blocking edge supports inspection, never automatic cancellation.
use super::{AdvisorInput, CATALOG_MAX_AGE_MS, Suggestion, suggestion};
/// Require repeated, fresh complete blocking observations before producing advice.
pub(super) fn evaluate(input: &AdvisorInput) -> Vec<Suggestion> {
    if !input
        .activity_coverage
        .fresh(input.now_ms, CATALOG_MAX_AGE_MS)
    {
        return Vec::new();
    }
    input.locks.iter().filter(|edge| edge.samples >= 3 && !edge.blockers.is_empty()).map(|edge| {
        let mut card = suggestion("pg.lock-hotspot", &edge.identity, format!(
            "Session {} blocked by {} at wait {}; same statement/edge in {} consecutive complete activity snapshots; transaction age {} ms (not wait duration).",
            edge.pid, edge.blockers, edge.wait, edge.samples,
            edge.transaction_ms.map_or_else(|| "unknown".into(), |age| format!("{age:.0}"))), input.activity_coverage.collected_ms);
        card.evidence_link = format!("sessions:{}", edge.pid);
        card.impact = "Inspect long transactions and application lock/access order".into();
        card.trade_offs = "Activity snapshots can miss transient blockers. Review the whole chain and owning transaction; cancellation stays an explicit safety-gated session action.".into();
        card
    }).collect()
}

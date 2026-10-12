// SPDX-License-Identifier: Apache-2.0
//! Statement counters prove spills and call rates, never application call paths.
use super::{AdvisorInput, Suggestion, suggestion};
use crate::{SampleState, StatementInterval};
use std::collections::BTreeMap;

/// Evaluate real contiguous statement intervals without inferred request-level traces.
pub(super) fn evaluate(input: &AdvisorInput) -> Vec<Suggestion> {
    let mut groups: BTreeMap<&str, Vec<&StatementInterval>> = BTreeMap::new();
    for interval in &input.intervals {
        groups
            .entry(&interval.statement_id)
            .or_default()
            .push(interval);
    }
    let mut out = Vec::new();
    for (id, mut intervals) in groups {
        intervals.sort_by(|a, b| a.collected_ms.total_cmp(&b.collected_ms));
        let Some(latest) = intervals.last() else {
            continue;
        };
        if latest.collected_ms > input.now_ms
            || input.now_ms - latest.collected_ms > 120_000.0
            || latest.state != SampleState::Ok
        {
            continue;
        }
        out.extend(spill(id, latest));
        out.extend(batching(id, &intervals));
    }
    out
}
fn spill(id: &str, interval: &StatementInterval) -> Option<Suggestion> {
    let writes = super::finite(interval.temp_written);
    let reads = super::finite(interval.temp_read);
    if !writes.is_some_and(|v| v > 0.0) && !reads.is_some_and(|v| v > 0.0) {
        return None;
    }
    let writes = writes.map_or_else(|| "unknown".into(), |v| format!("{v:.0}"));
    let reads = reads.map_or_else(|| "unknown".into(), |v| format!("{v:.0}"));
    let mut card = suggestion(
        "pg.statement-spill",
        id,
        format!(
            "Statement {id}: {writes} temporary block writes / {reads} reads in {:.0} ms interval ending {:.0}. Scope is statement; these are blocks, not bytes.",
            interval.duration_ms, interval.collected_ms
        ),
        interval.collected_ms,
    );
    card.confidence = "high".into();
    card.statement_id = Some(id.into());
    card.evidence_link = format!("statements:{id}");
    super::plan_rules::spill_notes(&mut card);
    Some(card)
}
fn batching(id: &str, intervals: &[&StatementInterval]) -> Option<Suggestion> {
    let recent = intervals.get(intervals.len().checked_sub(3)?..)?;
    if !recent.iter().all(|i| {
        i.state == SampleState::Ok
            && i.duration_ms > 0.0
            && i.mean_ms().is_some_and(|mean| mean <= 10.0)
            && i.calls
                .is_some_and(|calls| calls * 60_000.0 / i.duration_ms >= 1_000.0)
    }) {
        return None;
    }
    if !recent
        .windows(2)
        .all(|p| (p[1].collected_ms - p[1].duration_ms - p[0].collected_ms).abs() < 1_000.0)
    {
        return None;
    }
    let latest = recent.last()?;
    let mut card = suggestion(
        "pg.batching",
        id,
        format!(
            "Statement {id}: ≥ 1,000 calls/min and mean ≤ 10 ms in three consecutive complete intervals ending {:.0}; latest {:.0} calls / {:.0} ms, mean {:.2} ms.",
            latest.collected_ms,
            latest.calls?,
            latest.duration_ms,
            latest.mean_ms()?
        ),
        latest.collected_ms,
    );
    card.confidence = "low".into();
    card.statement_id = Some(id.into());
    card.evidence_link = format!("statements:{id}");
    card.impact = "Investigate batching or application caching".into();
    card.trade_offs = "Statistics cannot prove request-level N+1 or identify the application call path. Review request traces and cache correctness; no rewrite SQL is fabricated.".into();
    Some(card)
}

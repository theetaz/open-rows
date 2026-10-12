// SPDX-License-Identifier: Apache-2.0
//! Maintenance proposals require fresh catalog evidence and explicit age/preconditions.
use super::{
    AdvisorInput, CATALOG_MAX_AGE_MS, IndexEvidence, Suggestion, TableEvidence, catalog_fresh,
    finite, qualified, quote, suggestion,
};

const DAY_MS: f64 = 86_400_000.0;
/// Evaluate maintenance/index rules only against complete fresh catalog evidence.
pub(super) fn evaluate(input: &AdvisorInput) -> Vec<Suggestion> {
    let mut out = Vec::new();
    if catalog_fresh(input) {
        for table in &input.tables {
            out.extend(stale_statistics(table, input.tables_coverage.collected_ms));
            out.extend(vacuum(table, input.tables_coverage.collected_ms));
        }
    }
    if input
        .indexes_coverage
        .fresh(input.now_ms, CATALOG_MAX_AGE_MS)
    {
        for index in &input.indexes {
            out.extend(unused(index, input.indexes_coverage.collected_ms));
        }
        out.extend(duplicates(input));
    }
    out
}
fn stale_statistics(table: &TableEvidence, collected: f64) -> Option<Suggestion> {
    let (age, changed, live) = (
        finite(table.analyze_age_ms)?,
        finite(table.modifications)?,
        finite(table.live_tuples)?,
    );
    if age < DAY_MS || changed < 500.0 || changed < live * 0.1 {
        return None;
    }
    let mut card = suggestion(
        "pg.stale-statistics",
        &table.id,
        format!(
            "{}.{}: last analyze age {age:.0} ms ≥ 24 h; {changed:.0} modifications ≥ 500 and ≥ 10% of {live:.0} estimated live tuples.",
            table.schema, table.name
        ),
        collected,
    );
    card.sql = Some(format!("ANALYZE {};", qualified(table)));
    card.evidence_link = format!("tables:{}", table.id);
    card.impact = "Refresh planner estimates after substantial modifications".into();
    card.trade_offs = "ANALYZE samples the table and uses I/O/CPU; inspect skew and correlated predicates. Review production scheduling and locks.".into();
    Some(card)
}
fn vacuum(table: &TableEvidence, collected: f64) -> Option<Suggestion> {
    let (dead, live, age) = (
        finite(table.dead_tuples)?,
        finite(table.live_tuples)?,
        finite(table.vacuum_age_ms)?,
    );
    if dead < 100_000.0 || dead / (dead + live) < 0.2 || age < DAY_MS || table.samples < 3 {
        return None;
    }
    let mut card = suggestion(
        "pg.vacuum",
        &table.id,
        format!(
            "{}.{}: {dead:.0} estimated dead / {:.0} total tuples ≥ 20%; last vacuum age {age:.0} ms ≥ 24 h; {} consecutive samples. This is not measured byte bloat.",
            table.schema,
            table.name,
            dead + live,
            table.samples
        ),
        collected,
    );
    card.sql = Some(format!("VACUUM {};", qualified(table)));
    card.confidence = "low".into();
    card.evidence_link = format!("tables:{}", table.id);
    card.impact = "Make dead-tuple space reusable and reduce maintenance backlog".into();
    card.trade_offs = "Inspect autovacuum and long transactions first. VACUUM uses I/O and locks; it does not guarantee file shrinkage. Never use VACUUM FULL automatically.".into();
    Some(card)
}
fn unused(index: &IndexEvidence, collected: f64) -> Option<Suggestion> {
    let age = finite(index.reset_age_ms)?;
    if index.protected || !index.usable || finite(index.scans)? != 0.0 || age < DAY_MS * 7.0 {
        return None;
    }
    let mut card = suggestion(
        "pg.unused-index",
        &index.id,
        format!(
            "{}.{}: idx_scan = 0; known database reset age {age:.0} ms ≥ 7 days. New indexes may have shorter observation coverage.",
            index.schema, index.name
        ),
        collected,
    );
    card.sql = Some(format!(
        "DROP INDEX CONCURRENTLY {}.{};",
        quote(&index.schema),
        quote(&index.name)
    ));
    card.confidence = "low".into();
    card.evidence_link = format!("indexes:{}", index.id);
    card.impact = "Potentially reduce index storage and write amplification".into();
    card.trade_offs = "Zero scans does not prove unused: review index creation age, representative workload, seasonal queries, replicas and reset gaps. Confirm constraint/unique/PK dependencies again before running; dropping can regress queries and still takes locks.".into();
    Some(card)
}
fn duplicates(input: &AdvisorInput) -> Vec<Suggestion> {
    use std::collections::BTreeMap;
    use std::ops::Bound::{Excluded, Unbounded};
    let mut definitions = BTreeMap::<(&str, &str), &IndexEvidence>::new();
    let mut keys = BTreeMap::<(&str, &[String]), &IndexEvidence>::new();
    let indexes: Vec<_> = input
        .indexes
        .iter()
        .filter(|i| i.usable && !i.signature.is_empty())
        .collect();
    for index in &indexes {
        let winner = definitions
            .entry((&index.table_id, &index.signature))
            .or_insert(index);
        if (index.protected && !winner.protected)
            || (index.protected == winner.protected && index.id < winner.id)
        {
            *winner = index;
        }
        keys.entry((&index.table_id, &index.keys)).or_insert(index);
    }
    let mut out = Vec::new();
    for candidate in indexes.iter().filter(|i| !i.protected) {
        let Some(other) =
            definitions.get(&(candidate.table_id.as_str(), candidate.signature.as_str()))
        else {
            continue;
        };
        if other.id != candidate.id {
            out.push(duplicate(
                candidate,
                other,
                input.indexes_coverage.collected_ms,
            ));
        } else if let Some((_, other)) = keys
            .range((
                Excluded((candidate.table_id.as_str(), candidate.keys.as_slice())),
                Unbounded,
            ))
            .next()
        {
            out.extend(overlap(
                candidate,
                other,
                input.indexes_coverage.collected_ms,
            ));
        }
    }
    out
}
fn duplicate(candidate: &IndexEvidence, other: &IndexEvidence, collected: f64) -> Suggestion {
    let mut card = suggestion(
        "pg.duplicate-index",
        &candidate.id,
        format!(
            "{}.{} duplicates {}: ordered keys/includes, predicate, expressions, uniqueness, method, opclasses, collation and ordering signatures match ({}).",
            candidate.schema, candidate.name, other.name, candidate.signature
        ),
        collected,
    );
    card.sql = Some(format!(
        "DROP INDEX CONCURRENTLY {}.{};",
        quote(&candidate.schema),
        quote(&candidate.name)
    ));
    card.evidence_link = format!("indexes:{}", candidate.id);
    card.impact = "Remove redundant index write/storage cost".into();
    card.trade_offs = "Review dependencies, index validity and definition again. Drop only one equivalent index; concurrent drop still needs locks and cannot run in a transaction.".into();
    card
}
fn overlap(candidate: &IndexEvidence, other: &IndexEvidence, collected: f64) -> Option<Suggestion> {
    if other.table_id != candidate.table_id
        || candidate.keys.is_empty()
        || other.keys.len() <= candidate.keys.len()
        || !other.keys.starts_with(&candidate.keys)
    {
        return None;
    }
    let mut card = suggestion(
        "pg.overlapping-index",
        &candidate.id,
        format!(
            "{}.{} ordered key names {:?} are a prefix of {} {:?}. This does not establish equivalent opclasses, include coverage or ordering.",
            candidate.schema, candidate.name, candidate.keys, other.name, other.keys
        ),
        collected,
    );
    card.confidence = "low".into();
    card.evidence_link = format!("indexes:{}", candidate.id);
    card.impact = "Inspect potentially overlapping index maintenance cost".into();
    card.trade_offs = "Compare exact definitions, sort direction, collations, includes and query plans. A smaller index can be faster; no drop SQL is generated from a prefix alone.".into();
    Some(card)
}

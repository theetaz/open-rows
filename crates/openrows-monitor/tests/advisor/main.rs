// SPDX-License-Identifier: Apache-2.0
//! Every versioned rule has positive, negative, missing and stale evidence coverage.
mod support;
use openrows_monitor::advisor::*;
use support::{contains, fixture, missing, negative};

macro_rules! rule_tests {
    ($name:ident, $rule:literal) => {
        mod $name {
            use super::*;
            #[test]
            fn positive_evidence() {
                let input = fixture($rule);
                let cards = advise(&input);
                let card = cards
                    .iter()
                    .find(|card| card.rule_id == $rule)
                    .expect("rule evidence");
                assert_eq!(card.version, 1);
                assert!(!card.evidence.is_empty());
                assert!(!card.trade_offs.is_empty());
                assert!(!card.evidence_link.is_empty());
                assert_eq!(cards, advise(&input));
            }
            #[test]
            fn negative_evidence() {
                let mut input = fixture($rule);
                negative(&mut input, $rule);
                assert!(!contains(&input, $rule));
            }
            #[test]
            fn missing_evidence() {
                let mut input = fixture($rule);
                missing(&mut input, $rule);
                assert!(!contains(&input, $rule));
            }
            #[test]
            fn stale_evidence() {
                let mut input = fixture($rule);
                input.now_ms += 600_001.0;
                assert!(!contains(&input, $rule));
            }
        }
    };
}
rule_tests!(selective_scan, "pg.selective-scan");
rule_tests!(join_key, "pg.join-key");
rule_tests!(estimate_error, "pg.estimate-error");
rule_tests!(plan_spill, "pg.plan-spill");
rule_tests!(stale_statistics, "pg.stale-statistics");
rule_tests!(vacuum, "pg.vacuum");
rule_tests!(unused_index, "pg.unused-index");
rule_tests!(duplicate_index, "pg.duplicate-index");
rule_tests!(overlapping_index, "pg.overlapping-index");
rule_tests!(statement_spill, "pg.statement-spill");
rule_tests!(batching, "pg.batching");
rule_tests!(lock_hotspot, "pg.lock-hotspot");

#[test]
fn scan_requires_large_selective_table_and_existing_leading_index_suppresses_it() {
    let mut input = fixture("pg.selective-scan");
    input.tables[0].estimated_rows = Some(99_999.0);
    assert!(!contains(&input, "pg.selective-scan"));
    input.tables[0].estimated_rows = Some(200_000.0);
    input.indexes.push(support::index("existing"));
    assert!(!contains(&input, "pg.selective-scan"));
    input.indexes[0].usable = false;
    assert!(contains(&input, "pg.selective-scan"));
}
#[test]
fn drop_never_targets_primary_unique_constraint_or_invalid_indexes() {
    for rule in ["pg.unused-index", "pg.duplicate-index"] {
        let mut input = fixture(rule);
        for index in &mut input.indexes {
            index.protected = true;
        }
        assert!(!contains(&input, rule));
        for index in &mut input.indexes {
            index.protected = false;
            index.usable = false;
        }
        assert!(!contains(&input, rule));
    }
}
#[test]
fn exact_sql_quotes_identifiers_and_never_changes_server_settings() {
    let mut input = fixture("pg.selective-scan");
    input.tables[0].name = "odd\"table".into();
    let cards = advise(&input);
    assert_eq!(
        cards[0].sql.as_deref(),
        Some("CREATE INDEX CONCURRENTLY ON \"public\".\"odd\"\"table\" (\"key\");")
    );
    for rule in [
        "pg.plan-spill",
        "pg.statement-spill",
        "pg.batching",
        "pg.overlapping-index",
    ] {
        assert!(
            advise(&fixture(rule))
                .iter()
                .all(|card| card.rule_id != rule || card.sql.is_none())
        );
    }
}
#[test]
fn unexecuted_measured_nodes_and_zero_on_both_sides_do_not_produce_estimate_error() {
    let mut input = fixture("pg.estimate-error");
    input.plans[0].nodes[0].loops = Some(0.0);
    assert!(!contains(&input, "pg.estimate-error"));
    input.plans[0].nodes[0].loops = Some(1.0);
    input.plans[0].nodes[0].estimated_rows = Some(0.0);
    assert!(contains(&input, "pg.estimate-error"));
    input.plans[0].nodes[0].actual_rows = Some(0.0);
    assert!(!contains(&input, "pg.estimate-error"));
}

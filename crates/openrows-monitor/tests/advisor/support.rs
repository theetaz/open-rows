// SPDX-License-Identifier: Apache-2.0
//! Shared rule evidence is synthetic, typed and deliberately free of SQL literals.
use openrows_monitor::{SampleState, StatementInterval, advisor::*};

/// Complete synthetic evidence for one rule.
pub fn fixture(rule: &str) -> AdvisorInput {
    let mut input = AdvisorInput {
        now_ms: 600_000.0,
        tables_coverage: Coverage {
            collected_ms: 600_000.0,
            complete: true,
        },
        indexes_coverage: Coverage {
            collected_ms: 600_000.0,
            complete: true,
        },
        tables: vec![TableEvidence {
            id: "t".into(),
            schema: "public".into(),
            name: "events".into(),
            estimated_rows: Some(200_000.0),
            size_bytes: Some(8_000_000.0),
            live_tuples: Some(200_000.0),
            dead_tuples: Some(0.0),
            samples: 3,
            ..Default::default()
        }],
        ..Default::default()
    };
    match rule {
        "pg.selective-scan" | "pg.join-key" | "pg.estimate-error" | "pg.plan-spill" => {
            input.plans.push(plan(rule));
        }
        "pg.lock-hotspot" => {
            input.activity_coverage = input.tables_coverage.clone();
            input.locks.push(LockEvidence {
                identity: "7:start:8".into(),
                pid: "7".into(),
                blockers: "8".into(),
                wait: "Lock:transactionid".into(),
                transaction_ms: Some(2000.0),
                samples: 3,
            });
        }
        "pg.stale-statistics" => {
            input.tables[0].analyze_age_ms = Some(86_400_000.0);
            input.tables[0].modifications = Some(30_000.0);
        }
        "pg.vacuum" => {
            input.tables[0].dead_tuples = Some(100_000.0);
            input.tables[0].vacuum_age_ms = Some(86_400_000.0);
        }
        "pg.unused-index" => {
            input.indexes = vec![index("a")];
            input.indexes[0].reset_age_ms = Some(604_800_000.0);
        }
        "pg.duplicate-index" => {
            input.indexes = vec![index("a"), index("b")];
        }
        "pg.overlapping-index" => {
            input.indexes = vec![index("a"), index("b")];
            input.indexes[1].keys.push("other".into());
            input.indexes[1].signature = "longer".into();
        }
        "pg.statement-spill" | "pg.batching" => {
            input.intervals = (1..=3)
                .map(|i| interval(f64::from(i) * 15_000.0 + 555_000.0))
                .collect();
            if rule == "pg.statement-spill" {
                input.intervals[2].temp_written = Some(10.0);
            }
        }
        _ => unreachable!("callers select a known fixture rule"),
    }
    input
}
/// Ordinary unprotected btree index evidence.
pub fn index(id: &str) -> IndexEvidence {
    IndexEvidence {
        id: id.into(),
        table_id: "t".into(),
        schema: "public".into(),
        name: format!("events_{id}"),
        keys: vec!["key".into()],
        signature: "exact-definition".into(),
        usable: true,
        scans: Some(0.0),
        ..Default::default()
    }
}
/// A complete real-delta interval fixture.
pub fn interval(end: f64) -> StatementInterval {
    StatementInterval {
        statement_id: "q".into(),
        collected_ms: end,
        duration_ms: 15_000.0,
        state: SampleState::Ok,
        calls: Some(250.0),
        total_ms: Some(1000.0),
        rows: Some(250.0),
        shared_hit: Some(1.0),
        shared_read: Some(0.0),
        temp_written: Some(0.0),
        temp_read: Some(0.0),
    }
}
/// Check one rule without coupling to unrelated suggestions.
pub fn contains(input: &AdvisorInput, rule: &str) -> bool {
    advise(input).iter().any(|card| card.rule_id == rule)
}
/// Lower one precondition below its trigger.
pub fn negative(input: &mut AdvisorInput, rule: &str) {
    match rule {
        "pg.selective-scan" => input.plans[0].nodes[0].estimated_rows = Some(100_000.0),
        "pg.join-key" => input.indexes.push(index("existing")),
        "pg.estimate-error" => input.plans[0].nodes[0].actual_rows = Some(2.0),
        "pg.plan-spill" => input.plans[0].nodes[0].spill = false,
        "pg.stale-statistics" => input.tables[0].modifications = Some(10.0),
        "pg.vacuum" => input.tables[0].samples = 2,
        "pg.lock-hotspot" => input.locks[0].samples = 2,
        "pg.unused-index" => input.indexes[0].protected = true,
        "pg.duplicate-index" => input.indexes[1].signature = "other".into(),
        "pg.overlapping-index" => input.indexes[1].keys.reverse(),
        "pg.statement-spill" => input.intervals[2].temp_written = Some(0.0),
        "pg.batching" => input.intervals[1].calls = Some(1.0),
        _ => unreachable!("callers select a known fixture rule"),
    }
}
/// Remove the evidence required by a rule.
pub fn missing(input: &mut AdvisorInput, rule: &str) {
    match rule {
        "pg.join-key" => input.plans[0].nodes[0].join_columns.clear(),
        "pg.estimate-error" => input.plans[0].nodes[0].actual_rows = None,
        "pg.plan-spill" => input.plans[0].measured = false,
        "pg.stale-statistics" => input.tables[0].analyze_age_ms = None,
        "pg.vacuum" => input.tables[0].vacuum_age_ms = None,
        "pg.lock-hotspot" => input.activity_coverage.complete = false,
        "pg.unused-index" => input.indexes[0].reset_age_ms = None,
        "pg.selective-scan" | "pg.duplicate-index" | "pg.overlapping-index" => {
            input.indexes_coverage.complete = false;
        }
        "pg.statement-spill" => input.intervals[2].temp_written = None,
        "pg.batching" => input.intervals[1].state = SampleState::Gap,
        _ => unreachable!("callers select a known fixture rule"),
    }
}

fn plan(rule: &str) -> PlanEvidence {
    PlanEvidence {
        statement_id: "q".into(),
        collected_ms: 600_000.0,
        measured: rule != "pg.selective-scan" && rule != "pg.join-key",
        nodes: vec![NodeEvidence {
            id: 1,
            table_id: Some("t".into()),
            operator: "Seq Scan".into(),
            estimated_rows: Some(1.0),
            actual_rows: Some(100.0),
            loops: Some(1.0),
            filter_column: (rule == "pg.selective-scan").then(|| "key".into()),
            join_columns: if rule == "pg.join-key" {
                vec!["key".into()]
            } else {
                vec![]
            },
            spill: rule == "pg.plan-spill",
        }],
    }
}

// SPDX-License-Identifier: Apache-2.0
//! Opt-in bounded advisor benchmark, independent of Tauri and database I/O.
use openrows_monitor::advisor::{AdvisorInput, Coverage, IndexEvidence, advise};
use std::time::Instant;
#[test]
fn advisor_thousand_rows_opt_in() {
    if std::env::var_os("OPENROWS_BENCH").is_none() {
        return;
    }
    let mut input = AdvisorInput {
        now_ms: 0.0,
        indexes_coverage: Coverage {
            complete: true,
            collected_ms: 0.0,
        },
        indexes: (0..1000)
            .map(|i| IndexEvidence {
                id: i.to_string(),
                table_id: "t".into(),
                schema: "public".into(),
                name: format!("index_{i}"),
                keys: vec![format!("column_{i}")],
                signature: format!("signature_{i}"),
                usable: true,
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    };
    report("distinct", &input);
    for index in &mut input.indexes {
        index.keys = vec!["duplicate_key".into()];
        index.signature = "duplicate_definition".into();
    }
    report("duplicate", &input);
}

#[expect(clippy::print_stdout, reason = "opt-in benchmark timings only")]
fn report(case: &str, input: &AdvisorInput) {
    let mut timings = Vec::with_capacity(100);
    for _ in 0..100 {
        let start = Instant::now();
        std::hint::black_box(advise(std::hint::black_box(input)));
        timings.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    timings.sort_by(f64::total_cmp);
    println!(
        "advisor {case}: 1000 catalog rows, 100 iterations; median {:.3} ms; p95 {:.3} ms; ADR 1000-row analysis budget 10 ms p95",
        timings[50], timings[95]
    );
}

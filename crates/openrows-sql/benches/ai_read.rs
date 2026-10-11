// SPDX-License-Identifier: Apache-2.0
//! Opt-in model read-admission microbenchmarks; no normal-suite timing assertions.
use criterion::Criterion;
use openrows_sql::{Dialect, validate_ai_read};
use std::hint::black_box;
fn main() {
    if std::env::var("OPENROWS_BENCH").as_deref() != Ok("1") {
        return;
    }
    let mut criterion = Criterion::default().configure_from_args();
    for (name, sql) in [
        (
            "safe_select",
            "SELECT id,email FROM public.items WHERE id IN (1,2) LIMIT 20",
        ),
        (
            "refused_side_effect",
            "SELECT pg_catalog.pg_terminate_backend(1)",
        ),
    ] {
        criterion.bench_function(name, |bencher| {
            bencher.iter(|| validate_ai_read(black_box(sql), Dialect::Postgres));
        });
    }
    criterion.final_summary();
}

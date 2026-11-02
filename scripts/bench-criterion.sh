#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Run the Criterion benchmarks that are compared against `main` (docs/13 §4,
# docs/25 §5.1). Arguments go to Criterion: `--save-baseline main` on the base
# commit, `--baseline-lenient main` on the change, then scripts/bench_compare.py.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
benches=(openrows-results:result_window_encode openrows-driver-postgres:decode_pg
    openrows-sql:sql_split openrows-schema:schema_catalog)
for bench in "${benches[@]}"; do
    cargo bench --locked -p "${bench%%:*}" --bench "${bench##*:}" -- "$@"
done

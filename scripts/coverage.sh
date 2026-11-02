#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Line coverage of openrows-core, openrows-sql and the drivers' value codecs with
# cargo-llvm-cov, then the >= 80 % gate per group (NFR-MAINT-03, docs/17 §2.7).
# Container suites skip without their environment; the codecs are covered by unit tests.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
if ! cargo llvm-cov --version >/dev/null 2>&1; then
    echo "cargo-llvm-cov is missing: cargo install cargo-llvm-cov (needs the llvm-tools-preview component)" >&2
    exit 2
fi
packages=(-p openrows-core -p openrows-sql -p openrows-driver-postgres -p openrows-driver-mysql
    -p openrows-driver-mssql -p openrows-driver-sqlite)
cargo llvm-cov nextest --locked --no-report "${packages[@]}"
report=$(mktemp)
trap 'rm -f "$report"' EXIT
cargo llvm-cov report --summary-only --json --output-path "$report"
python3 -B scripts/coverage_gate.py "$report"

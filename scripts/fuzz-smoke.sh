#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Run every cargo-fuzz target in fuzz/ for a few seconds each. Skips (exit 0)
# when no nightly toolchain or cargo-fuzz is installed; the fuzz workspace is
# excluded from the stable workspace, so nothing else builds it.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
seconds="${1:-20}"
if ! [[ "$seconds" =~ ^[1-9][0-9]*$ ]]; then
    echo "usage: just fuzz-smoke <seconds per target>, e.g. 20" >&2
    exit 2
fi
# Never let rustup install a toolchain as a side effect of probing for one.
export RUSTUP_AUTO_INSTALL=0
if ! rustup toolchain list 2>/dev/null | grep -q '^nightly' || ! cargo +nightly --version >/dev/null 2>&1; then
    echo "SKIP: no nightly toolchain (rustup toolchain install nightly); fuzz targets not run"
    exit 0
fi
if ! cargo +nightly fuzz --version >/dev/null 2>&1; then
    echo "SKIP: cargo-fuzz is not installed (cargo install cargo-fuzz); fuzz targets not run"
    exit 0
fi
targets="$(cargo +nightly fuzz list --fuzz-dir fuzz)"
for target in $targets; do
    echo "fuzz: $target for ${seconds}s"
    cargo +nightly fuzz run --fuzz-dir fuzz "$target" -- -max_total_time="$seconds"
done

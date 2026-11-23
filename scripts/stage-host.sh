#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Build only the production helpers (driver host and the openrows-mcp agent bridge),
# then stage Tauri's target-suffixed externalBin files.
# Usage: stage-host.sh [release|dev|custom-profile] [target-triple]
# Respects CARGO_TARGET_DIR, RUSTC_WRAPPER and CARGO_BUILD_TARGET; no test features.
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"
profile=${1:-release}
requested_target=${2:-${CARGO_BUILD_TARGET:-}}
target=${requested_target:-$(rustc -vV | sed -n 's/^host: //p')}
if [[ -z "$target" || ! "$profile" =~ ^[a-zA-Z0-9_-]+$ || ! "$target" =~ ^[a-zA-Z0-9_-]+$ ]]; then
    printf 'stage-host: invalid profile or target\n' >&2
    exit 2
fi
# Keep native and cross-target output identical. CI exports CARGO_BUILD_TARGET
# job-wide so staging, Clippy and nextest reuse the same bundled native library.
output="${CARGO_TARGET_DIR:-target}/$target"
profile_dir=$profile
[[ "$profile" != dev ]] || profile_dir=debug
suffix=
[[ "$target" != *-windows-* ]] || suffix=.exe
mkdir -p apps/desktop/src-tauri/binaries
# stage <package> <binary> [cargo options...]: build one helper and copy it beside the app.
stage() {
    local package=$1 binary=$2
    shift 2
    nice -n 10 bash scripts/sccache-guard.sh cargo build --locked -p "$package" --bin "$binary" "$@" \
        --profile "$profile" --target "$target"
    local source_binary="$output/$profile_dir/$binary$suffix"
    local destination="apps/desktop/src-tauri/binaries/$binary-$target$suffix"
    cp "$source_binary" "$destination"
    chmod +x "$destination"
    printf 'stage-host: production %s: %s -> %s\n' "$profile" "$source_binary" "$destination"
}
stage openrows-driver-host openrows-driver-host --no-default-features --features duckdb
stage openrows-mcp openrows-mcp

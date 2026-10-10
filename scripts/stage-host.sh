#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Build only the production helper, then stage Tauri's target-suffixed externalBin.
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
options=(--locked -p openrows-driver-host --bin openrows-driver-host --no-default-features --profile "$profile")
output=${CARGO_TARGET_DIR:-target}
if [[ -n "$requested_target" ]]; then
    options+=(--target "$target")
    output="$output/$target"
fi
profile_dir=$profile
[[ "$profile" != dev ]] || profile_dir=debug
suffix=
[[ "$target" != *-windows-* ]] || suffix=.exe
nice -n 10 bash scripts/sccache-guard.sh cargo build "${options[@]}"
source_binary="$output/$profile_dir/openrows-driver-host$suffix"
destination="apps/desktop/src-tauri/binaries/openrows-driver-host-$target$suffix"
mkdir -p apps/desktop/src-tauri/binaries
cp "$source_binary" "$destination"
chmod +x "$destination"
printf 'stage-host: production %s -> %s\n' "$profile" "$destination"

#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Run a command after making sure an sccache RUSTC_WRAPPER works in this process context.
# An sccache server keeps the sandbox of the process that spawned it, and a sandbox without
# localhost network cannot reach any server; both fail with "Operation not permitted".
# See docs/18-repository-and-dev-workflow.md §2.2.
set -euo pipefail

# Compile an empty crate through the server into the Cargo target directory, so the server
# itself must write where Cargo will write.
probe_wrapper() {
    local target dir status=0
    target=${CARGO_TARGET_DIR:-$PWD/target}
    mkdir -p "$target" && dir=$(mktemp -d "$target/.sccache-probe.XXXXXX") || return 1
    : >"$dir/probe.rs"
    "$RUSTC_WRAPPER" "${RUSTC:-rustc}" --crate-name=sccache_probe --crate-type=lib \
        --edition=2024 --emit=metadata --out-dir "$dir" "$dir/probe.rs" >/dev/null 2>&1 || status=$?
    rm -rf "$dir"
    return "$status"
}

restart_server() {
    "$RUSTC_WRAPPER" --stop-server >/dev/null 2>&1 || true
    "$RUSTC_WRAPPER" --start-server >/dev/null 2>&1
}

if [[ "$(basename -- "${RUSTC_WRAPPER:-none}")" == sccache ]] && ! probe_wrapper; then
    printf 'sccache-guard: server unusable here; restarting it in this context.\n' >&2
    if ! restart_server || ! probe_wrapper; then
        printf 'sccache-guard: sccache unreachable (sandbox without localhost network?); building without RUSTC_WRAPPER. See docs/18 §2.2.\n' >&2
        export RUSTC_WRAPPER=
    fi
fi
exec "$@"

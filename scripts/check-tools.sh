#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
set -euo pipefail
missing=0
check_tool() {
    if ! command -v "$1" >/dev/null 2>&1; then
        printf 'Missing %s. Install: %s\n' "$1" "$2" >&2
        missing=1
    fi
}
check_tool rustup 'https://rustup.rs'
check_tool cargo 'https://rustup.rs'
check_tool just 'cargo install just --locked'
check_tool cargo-nextest 'cargo install cargo-nextest --locked'
check_tool cargo-deny 'cargo install cargo-deny --locked'
check_tool cargo-machete 'cargo install cargo-machete --locked'
[[ "$missing" == 0 ]] || exit 1
if ! rustup run 1.94.0 cargo --version ||
    ! rustup run 1.94.0 rustfmt --version ||
    ! rustup run 1.94.0 cargo clippy --version; then
    printf '%s\n' 'Install: rustup toolchain install 1.94.0 --profile minimal --component rustfmt --component clippy' >&2
    exit 1
fi
printf 'Required tools and pinned toolchain available. sccache is optional.\n'

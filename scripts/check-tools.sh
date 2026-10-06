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
check_tool node 'Install Node.js >=24 from https://nodejs.org'
check_tool pnpm 'npm install --global pnpm@12.9.1'
[[ "$missing" == 0 ]] || exit 1
if ! rustup run 1.94.0 cargo --version ||
    ! rustup run 1.94.0 rustfmt --version ||
    ! rustup run 1.94.0 cargo clippy --version; then
    printf '%s\n' 'Install: rustup toolchain install 1.94.0 --profile minimal --component rustfmt --component clippy' >&2
    exit 1
fi
node --eval 'if (Number(process.versions.node.split(".")[0]) < 24) process.exit(1)' || {
    printf 'Node.js >=24 is required.\n' >&2
    exit 1
}
required_pnpm=$(node --eval 'process.stdout.write(require("./package.json").packageManager.split("@")[1])')
if [[ "$(pnpm --version)" != "$required_pnpm" ]]; then
    printf 'Install pnpm@%s with npm install --global pnpm@%s\n' "$required_pnpm" "$required_pnpm" >&2
    exit 1
fi
printf 'Required tools and pinned toolchain available. sccache is optional.\n'

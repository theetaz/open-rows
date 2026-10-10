#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Normal/build edges must preserve the production native-engine boundary.
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"
parent=$(cargo tree --locked -p openrows-app --edges normal,build --prefix none)
if rg -q '^(duckdb|libduckdb-sys) v' <<< "$parent"; then
    printf 'host graph: DuckDB leaked into the parent\n' >&2
    exit 1
fi
host=$(cargo tree --locked -p openrows-driver-host --no-default-features --features duckdb --edges normal,build --prefix none)
if ! rg -q '^duckdb v' <<< "$host" || ! rg -q '^libduckdb-sys v' <<< "$host"; then
    printf 'host graph: production helper lacks DuckDB\n' >&2
    exit 1
fi
if rg -q '^(openrows-driver-sqlite|rusqlite|libsqlite3-sys) v' <<< "$host"; then
    printf 'host graph: native SQLite leaked into the production helper\n' >&2
    exit 1
fi
for package in openrows-app openrows-driver-host; do
    options=(--locked -p "$package" --edges normal --prefix none)
    if [[ "$package" == openrows-driver-host ]]; then
        options+=(--no-default-features --features duckdb)
    fi
    runtime=$(cargo tree "${options[@]}")
    if rg -q '^webpki-roots v' <<< "$runtime"; then
        printf 'host graph: build-only CA data leaked into %s runtime\n' "$package" >&2
        exit 1
    fi
done
printf 'host graph: webpki-roots is absent from shipped runtime graphs\n'
printf 'host graph: parent has no DuckDB; production helper has DuckDB and no native SQLite\n'

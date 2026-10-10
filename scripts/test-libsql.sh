#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Authenticated, pinned sqld integration + capability conformance; no sleeps.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
image="${1:-ghcr.io/tursodatabase/libsql-server:v0.24.32}"
docker info >/dev/null
cargo build -p openrows-net --bin fake-cloud-hop --features test-helper --locked
# Resolve the built directory so credential validation also accepts relative Cargo targets.
export OPENROWS_FAKE_HELPER="$(cd -- "${CARGO_TARGET_DIR:-target}/debug" && pwd)/fake-cloud-hop"
openssl_bin="${OPENROWS_OPENSSL:-openssl}"
if [[ -x /opt/homebrew/opt/openssl@3/bin/openssl ]]; then
    openssl_bin=/opt/homebrew/opt/openssl@3/bin/openssl
fi
scratch="$(mktemp -d "$PWD/.libsql-test.XXXXXX")"
name="openrows-libsql-test-$$"
cleanup() {
    docker rm --force --volumes "$name" >/dev/null 2>&1 || true
    rm -rf "$scratch"
}
trap cleanup EXIT INT TERM
"$openssl_bin" genpkey -algorithm ED25519 -out "$scratch/key.pem" 2>/dev/null
"$openssl_bin" pkey -in "$scratch/key.pem" -pubout -out "$scratch/public.pem" 2>/dev/null
base64url() { "$openssl_bin" base64 -A | tr '+/' '-_' | tr -d '='; }
header="$(printf '%s' '{"alg":"EdDSA","typ":"JWT"}' | base64url)"
payload="$(printf '%s' '{"a":"rw"}' | base64url)"
printf '%s' "$header.$payload" > "$scratch/signing-input"
"$openssl_bin" pkeyutl -sign -rawin -inkey "$scratch/key.pem" -in "$scratch/signing-input" -out "$scratch/signature"
token="$header.$payload.$(base64url < "$scratch/signature")"
platform=()
case "$(uname -m)" in arm64 | aarch64) platform=(--platform linux/amd64) ;; esac
docker run --detach --rm --name "$name" --cpus 2 --memory 2g "${platform[@]}" --publish 127.0.0.1::8080 \
    --env SQLD_NODE=primary --env SQLD_AUTH_JWT_KEY="$(cat "$scratch/public.pem")" "$image" >/dev/null
port="$(docker port "$name" 8080/tcp | head -n 1 | sed 's/.*://')"
deadline=$((SECONDS + 90))
until curl --silent --fail --max-time 2 "http://127.0.0.1:$port/health" >/dev/null; do
    if (( SECONDS >= deadline )); then
        docker logs "$name"
        exit 1
    fi
done
export OPENROWS_TEST_LIBSQL_PORT="$port" OPENROWS_TEST_LIBSQL_TOKEN="$token"
echo "$image on 127.0.0.1:$port (Ed25519 JWT auth enabled)"
cargo nextest run -p openrows-driver-libsql --locked --test remote --run-ignored only
cargo nextest run -p openrows-driver-conformance --locked --test libsql --run-ignored only --test-threads 1
cargo nextest run -p openrows-driver-conformance --locked --test libsql_file

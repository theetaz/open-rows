#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Run the ignored PostgreSQL integration tests (driver, session actor, schema
# cache, the driver conformance suite, imports and dump/restore) against a throwaway container with
# TLS enabled. Skips (exit 0) when Docker is unavailable (fails with
# OPENROWS_REQUIRE_DOCKER=1); always removes the container and the generated
# certificates.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
version="${1:-17}"
if ! [[ "$version" =~ ^[0-9]+$ ]]; then
    echo "usage: just test-pg <major version>, e.g. 17" >&2
    exit 2
fi
if ! command -v docker >/dev/null 2>&1 || ! docker info >/dev/null 2>&1; then
    # CI sets OPENROWS_REQUIRE_DOCKER=1 so a missing daemon fails the job.
    if [ "${OPENROWS_REQUIRE_DOCKER:-0}" = 1 ]; then
        echo "FAIL: Docker is required (OPENROWS_REQUIRE_DOCKER=1) but not available" >&2
        exit 1
    fi
    echo "SKIP: Docker is not available; PostgreSQL integration tests not run"
    exit 0
fi
cargo build -p openrows-net --bin fake-cloud-hop --features test-helper --locked
export OPENROWS_FAKE_HELPER="${CARGO_TARGET_DIR:-target}/debug/fake-cloud-hop"
# Synthetic credential for a container bound to 127.0.0.1 and removed on exit.
password="openrows-test-only"
name="openrows-pg-test-$$"
tls_dir="$(mktemp -d "${TMPDIR:-/tmp}/openrows-pg-tls.XXXXXX")"
cleanup() {
    docker rm --force --volumes "$name" >/dev/null 2>&1 || true
    rm -rf "$tls_dir"
}
trap cleanup EXIT INT TERM
bash scripts/test-pg-certs.sh "$tls_dir"
# The server key must belong to the postgres user with mode 0600, so it is
# copied out of the read-only mount before the image's entrypoint starts.
ssl_dir=/etc/openrows-ssl
docker run --detach --name "$name" --publish 127.0.0.1::5432 \
    --env POSTGRES_PASSWORD="$password" \
    --volume "$tls_dir:/certs:ro" \
    --volume "$tls_dir/init-tls.sh:/docker-entrypoint-initdb.d/init-tls.sh:ro" \
    --entrypoint sh "postgres:${version}-alpine" -c "
        mkdir -p $ssl_dir &&
        install -o postgres -g postgres -m 600 /certs/server.key $ssl_dir/server.key &&
        install -o postgres -g postgres -m 644 /certs/server.crt /certs/ca.crt $ssl_dir/ &&
        exec docker-entrypoint.sh postgres -c ssl=on \
            -c ssl_cert_file=$ssl_dir/server.crt -c ssl_key_file=$ssl_dir/server.key \
            -c ssl_ca_file=$ssl_dir/ca.crt" >/dev/null
# The image's init server listens only on a Unix socket, so TCP readiness
# means the final server is up.
deadline=$((SECONDS + 60))
while (( SECONDS < deadline )); do
    if [ "$(docker inspect --format '{{.State.Running}}' "$name")" != true ]; then
        docker logs "$name" >&2
        exit 1
    fi
    if docker exec "$name" pg_isready --host 127.0.0.1 --quiet 2>/dev/null; then
        break
    fi
done
if ! docker exec "$name" pg_isready --host 127.0.0.1 --quiet; then
    docker logs "$name" >&2
    exit 1
fi
port="$(docker port "$name" 5432/tcp | head -n 1 | sed 's/.*://')"
echo "PostgreSQL ${version} ($(docker exec "$name" postgres --version)) on 127.0.0.1:${port}, TLS on"
OPENROWS_TEST_PG_PORT="$port" OPENROWS_TEST_PG_PASSWORD="$password" OPENROWS_TEST_PG_CONTAINER="$name" \
    OPENROWS_TEST_PG_TLS_DIR="$tls_dir" \
    cargo nextest run -p openrows-driver-postgres -p openrows-core -p openrows-schema \
    -p openrows-driver-conformance -p openrows-import -p openrows-dump --locked --test postgres \
    --run-ignored only --no-capture

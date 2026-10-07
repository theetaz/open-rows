#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Run the ignored PostgreSQL integration tests (driver and session actor) against
# a throwaway container.
# Skips (exit 0) when Docker is unavailable; always removes the container.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
version="${1:-17}"
if ! [[ "$version" =~ ^[0-9]+$ ]]; then
    echo "usage: just test-pg <major version>, e.g. 17" >&2
    exit 2
fi
if ! command -v docker >/dev/null 2>&1 || ! docker info >/dev/null 2>&1; then
    echo "SKIP: Docker is not available; PostgreSQL integration tests not run"
    exit 0
fi
# Synthetic credential for a container bound to 127.0.0.1 and removed on exit.
password="openrows-test-only"
name="openrows-pg-test-$$"
cleanup() { docker rm --force --volumes "$name" >/dev/null 2>&1 || true; }
trap cleanup EXIT INT TERM
docker run --detach --rm --name "$name" --publish 127.0.0.1::5432 \
    --env POSTGRES_PASSWORD="$password" "postgres:${version}-alpine" >/dev/null
# The image's init server listens only on a Unix socket, so TCP readiness
# means the final server is up.
for _ in $(seq 1 120); do
    if docker exec "$name" pg_isready --host 127.0.0.1 --quiet 2>/dev/null; then
        break
    fi
    sleep 0.5
done
docker exec "$name" pg_isready --host 127.0.0.1 --quiet
port="$(docker port "$name" 5432/tcp | head -n 1 | sed 's/.*://')"
echo "PostgreSQL ${version} ($(docker exec "$name" postgres --version)) on 127.0.0.1:${port}"
OPENROWS_TEST_PG_PORT="$port" OPENROWS_TEST_PG_PASSWORD="$password" \
    cargo nextest run -p openrows-driver-postgres -p openrows-core --locked --test postgres \
    --run-ignored only --no-capture

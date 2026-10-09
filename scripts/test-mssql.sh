#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Run the ignored SQL Server driver integration tests and the conformance
# suite against a throwaway container (default image
# mcr.microsoft.com/mssql/server:2022-latest). The image is amd64-only: on
# arm64 hosts it runs emulated (slower, but working).
# Skips (exit 0) when Docker is unavailable (fails with OPENROWS_REQUIRE_DOCKER=1);
# always removes the container.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
image="${1:-mcr.microsoft.com/mssql/server:2022-latest}"
if ! command -v docker >/dev/null 2>&1 || ! docker info >/dev/null 2>&1; then
    # CI sets OPENROWS_REQUIRE_DOCKER=1 so a missing daemon fails the job.
    if [ "${OPENROWS_REQUIRE_DOCKER:-0}" = 1 ]; then
        echo "FAIL: Docker is required (OPENROWS_REQUIRE_DOCKER=1) but not available" >&2
        exit 1
    fi
    echo "SKIP: Docker is not available; SQL Server integration tests not run"
    exit 0
fi
# Synthetic credential (meets the server's complexity rules) for a container
# bound to 127.0.0.1 and removed on exit.
password="Openrows-test-only1"
name="openrows-mssql-test-$$"
cleanup() {
    docker rm --force --volumes "$name" >/dev/null 2>&1 || true
}
trap cleanup EXIT INT TERM

platform=()
case "$(uname -m)" in
    arm64 | aarch64) platform=(--platform linux/amd64) ;;
esac
docker run --detach --rm --name "$name" "${platform[@]}" --publish 127.0.0.1::1433 \
    --env ACCEPT_EULA=Y --env MSSQL_SA_PASSWORD="$password" "$image" >/dev/null

sql() {
    docker exec "$name" /opt/mssql-tools18/bin/sqlcmd -C -b -S 127.0.0.1 -U sa \
        -P "$password" -Q "$1" >/dev/null 2>&1
}
# The server needs tens of seconds to start (minutes when emulated).
deadline=$((SECONDS + 300))
while (( SECONDS < deadline )); do
    if sql "SELECT 1"; then
        break
    fi
done
sql "IF DB_ID('openrows_test') IS NULL CREATE DATABASE openrows_test"
port="$(docker port "$name" 1433/tcp | head -n 1 | sed 's/.*://')"
echo "$image on 127.0.0.1:${port}"
export OPENROWS_TEST_MSSQL_PORT="$port" OPENROWS_TEST_MSSQL_PASSWORD="$password"
cargo nextest run -p openrows-driver-mssql -p openrows-import --locked --test sqlserver \
    --run-ignored only --no-capture
# One group at a time: SQL Server reuses session ids at once, and the
# process-list group signals a session id after ending it.
cargo nextest run -p openrows-driver-conformance --locked --test sqlserver \
    --run-ignored only --test-threads 1

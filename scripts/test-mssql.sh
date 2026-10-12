#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Run the ignored SQL Server driver integration tests and the conformance
# suite against a throwaway container. The default image is
# mcr.microsoft.com/mssql/server:2022-latest on x86_64 (CI) and the native
# mcr.microsoft.com/azure-sql-edge on arm64, so no host runs x86 emulation.
# Readiness and the test database come from a Rust test, not `sqlcmd`, which
# Azure SQL Edge does not ship.
# Skips (exit 0) when Docker is unavailable (fails with OPENROWS_REQUIRE_DOCKER=1);
# always removes the container.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
case "$(uname -m)" in
    arm64 | aarch64) default_image=mcr.microsoft.com/azure-sql-edge:latest ;;
    *) default_image=mcr.microsoft.com/mssql/server:2022-latest ;;
esac
image="${1:-$default_image}"
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

docker run --detach --rm --name "$name" --publish 127.0.0.1::1433 \
    --env ACCEPT_EULA=Y --env MSSQL_SA_PASSWORD="$password" "$image" >/dev/null

port="$(docker port "$name" 1433/tcp | head -n 1 | sed 's/.*://')"
echo "$image on 127.0.0.1:${port}"
export OPENROWS_TEST_MSSQL_PORT="$port" OPENROWS_TEST_MSSQL_PASSWORD="$password"
# Waits (bounded) for logins and creates openrows_test before the suites.
cargo nextest run -p openrows-driver-mssql --locked --test sqlserver \
    --run-ignored only -E 'test(/^ready::/)'
cargo nextest run -p openrows-driver-mssql -p openrows-import --locked --test sqlserver \
    --run-ignored only --no-capture
cargo nextest run -p openrows-core --locked --test sqlserver_explain \
    --run-ignored only --no-capture
# One group at a time: SQL Server reuses session ids at once, and the
# process-list group signals a session id after ending it.
cargo nextest run -p openrows-driver-conformance --locked --test sqlserver \
    --run-ignored only --test-threads 1

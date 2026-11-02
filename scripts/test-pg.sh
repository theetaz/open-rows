#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Run the ignored PostgreSQL integration tests (driver, session actor, schema
# cache, the driver conformance suite, imports and dump/restore) against a throwaway container with
# TLS enabled, for each requested major version in turn (default 17). After each main suite the
# monitoring phases run serially: server-wide statistics resets on the same container, then a
# container without shared_preload_libraries, then a streaming primary/replica pair (one suite).
# Every suite and phase runs even after a failure; the exit status reports any failure.
# Skips (exit 0) when Docker is unavailable (fails with OPENROWS_REQUIRE_DOCKER=1); always removes
# every container, the network and the generated certificates.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
if [ "$#" -eq 0 ]; then
    set -- 17
fi
for version in "$@"; do
    if ! [[ "$version" =~ ^[0-9]+$ ]]; then
        echo "usage: just test-pg [major version...], e.g. 17 or 13 15 16 17 18" >&2
        exit 2
    fi
done
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
# Resolve the built directory so credential validation also accepts relative Cargo targets.
helper_dir="${CARGO_TARGET_DIR:-target}"
if [ -n "${CARGO_BUILD_TARGET:-}" ]; then
    helper_target="${CARGO_BUILD_TARGET##*/}"
    helper_dir="$helper_dir/${helper_target%.json}"
fi
export OPENROWS_FAKE_HELPER="$(cd -- "$helper_dir/debug" && pwd)/fake-cloud-hop"
# Synthetic credential for containers bound to 127.0.0.1 and removed on exit.
password="openrows-test-only"
prefix="${OPENROWS_TEST_CONTAINER_PREFIX:-openrows}-pg-test-$$"
network="$prefix-net"
tls_dir="$(mktemp -d "$PWD/.pg-test-tls.XXXXXX")"
remove_containers() {
    docker ps --all --quiet --filter "name=$prefix-" | while read -r id; do
        docker rm --force --volumes "$id" >/dev/null 2>&1 || true
    done
    docker network rm "$network" >/dev/null 2>&1 || true
}
cleanup() {
    remove_containers
    rm -rf "$tls_dir"
}
trap cleanup EXIT INT TERM
bash scripts/test-pg-certs.sh "$tls_dir"
# Every suite and phase runs even after a failure; the script fails at the end.
status=0
record() {
    "$@" || status=1
}
# Phase-only tests: excluded from the parallel suite, each run alone in its phase.
phase_tests='test(/server_statistics_reset_|preload_absent_|streaming_replica_/)'

# Wait until a container's final server accepts TCP; the image's init server
# listens only on a Unix socket, so TCP readiness means the final server is up.
wait_ready() {
    local deadline=$((SECONDS + 90))
    while (( SECONDS < deadline )); do
        if [ "$(docker inspect --format '{{.State.Running}}' "$1")" != true ]; then
            docker logs "$1" >&2
            return 1
        fi
        if docker exec "$1" pg_isready --host 127.0.0.1 --quiet 2>/dev/null; then
            return 0
        fi
    done
    docker logs "$1" >&2
    return 1
}
host_port() {
    docker port "$1" 5432/tcp | head -n 1 | sed 's/.*://'
}
postgres_tests() {
    cargo nextest run -p openrows-driver-postgres --locked --test postgres --run-ignored only \
        --no-capture "$@"
}

main_suite() {
    local version="$1" name="$prefix-$1"
    # The server key must belong to the postgres user with mode 0600, so it is
    # copied out of the read-only mount before the image's entrypoint starts.
    local ssl_dir=/etc/openrows-ssl
    docker run --detach --name "$name" --publish 127.0.0.1::5432 \
        --env POSTGRES_PASSWORD="$password" \
        --volume "$tls_dir:/certs:ro" \
        --volume "$tls_dir/init-tls.sh:/docker-entrypoint-initdb.d/init-tls.sh:ro" \
        --entrypoint sh "postgres:${version}-alpine" -c "
            mkdir -p $ssl_dir &&
            install -o postgres -g postgres -m 600 /certs/server.key $ssl_dir/server.key &&
            install -o postgres -g postgres -m 644 /certs/server.crt /certs/ca.crt $ssl_dir/ &&
            exec docker-entrypoint.sh postgres -c shared_preload_libraries=pg_stat_statements -c ssl=on \
                -c ssl_cert_file=$ssl_dir/server.crt -c ssl_key_file=$ssl_dir/server.key \
                -c ssl_ca_file=$ssl_dir/ca.crt" >/dev/null
    wait_ready "$name"
    local port
    port="$(host_port "$name")"
    echo "PostgreSQL ${version} ($(docker exec "$name" postgres --version)) on 127.0.0.1:${port}, TLS on"
    export OPENROWS_TEST_PG_PORT="$port" OPENROWS_TEST_PG_PASSWORD="$password" \
        OPENROWS_TEST_PG_CONTAINER="$name" OPENROWS_TEST_PG_TLS_DIR="$tls_dir"
    record cargo nextest run -p openrows-driver-postgres -p openrows-core -p openrows-schema \
        -p openrows-mcp -p openrows-driver-conformance -p openrows-import -p openrows-dump --locked --test postgres \
        --run-ignored only --no-capture --no-fail-fast -E "not $phase_tests"
    echo "PostgreSQL ${version}: serial server-wide statistics reset phase"
    record postgres_tests -E 'test(/server_statistics_reset_/)'
    unset OPENROWS_TEST_PG_CONTAINER OPENROWS_TEST_PG_TLS_DIR
    remove_containers
}

preload_phase() {
    local version="$1" name="$prefix-$1-preload"
    docker run --detach --name "$name" --publish 127.0.0.1::5432 \
        --env POSTGRES_PASSWORD="$password" "postgres:${version}-alpine" >/dev/null
    wait_ready "$name"
    echo "PostgreSQL ${version}: phase preload-absent (no shared_preload_libraries)"
    OPENROWS_TEST_PG_PORT="$(host_port "$name")" OPENROWS_TEST_PG_PHASE=preload-absent \
        record postgres_tests -E 'test(/preload_absent_/)'
    remove_containers
}

replica_phase() {
    local version="$1" primary="$prefix-$1-primary" replica="$prefix-$1-replica"
    docker network create "$network" >/dev/null
    docker run --detach --name "$primary" --network "$network" --network-alias primary \
        --publish 127.0.0.1::5432 --env POSTGRES_PASSWORD="$password" \
        "postgres:${version}-alpine" >/dev/null
    wait_ready "$primary"
    docker exec "$primary" sh -c \
        'echo "host replication replicator all md5" >> "$PGDATA/pg_hba.conf"'
    docker exec "$primary" psql --quiet --username postgres \
        --command "CREATE ROLE replicator REPLICATION LOGIN PASSWORD '$password'" \
        --command "SELECT pg_reload_conf()" >/dev/null
    # The standby clones the primary as the unprivileged postgres user and then streams,
    # applying each commit 2 s late so replay lag is a stable, known quantity.
    docker run --detach --name "$replica" --network "$network" --publish 127.0.0.1::5432 \
        --user postgres --env PGPASSWORD="$password" --entrypoint sh "postgres:${version}-alpine" -c "
            mkdir -m 700 /tmp/replica &&
            pg_basebackup --host primary --username replicator --pgdata /tmp/replica \
                --write-recovery-conf --wal-method stream &&
            exec postgres -D /tmp/replica -c wal_receiver_status_interval=1 \
                -c recovery_min_apply_delay=2s" >/dev/null
    wait_ready "$replica"
    echo "PostgreSQL ${version}: phase replica (streaming primary/replica pair)"
    OPENROWS_TEST_PG_PORT="$(host_port "$primary")" \
        OPENROWS_TEST_PG_REPLICA_PORT="$(host_port "$replica")" OPENROWS_TEST_PG_PHASE=replica \
        record postgres_tests -E 'test(/streaming_replica_/)'
    remove_containers
}

for version in "$@"; do
    main_suite "$version"
    preload_phase "$version"
    replica_phase "$version"
done
exit "$status"

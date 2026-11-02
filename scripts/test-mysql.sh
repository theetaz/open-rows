#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Run the ignored MySQL/MariaDB driver integration tests and the conformance suite
# against throwaway containers, one image after another (default: mysql:8.4,
# mariadb:11.4, mysql:9.7 and pingcap/tidb:v8.5.1). Each database has TLS on and an
# OpenSSH container on its Docker network for the TLS and SSH groups. Every image
# runs even after a failure; the exit status reports any failure.
# Skips (exit 0) when Docker is unavailable (fails with OPENROWS_REQUIRE_DOCKER=1);
# always removes every container, network and generated certificate.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
source scripts/test-sshd.sh
if [ "$#" -eq 0 ]; then
    set -- mysql:8.4 mariadb:11.4 mysql:9.7 pingcap/tidb:v8.5.1
fi
if ! command -v docker >/dev/null 2>&1 || ! docker info >/dev/null 2>&1; then
    # CI sets OPENROWS_REQUIRE_DOCKER=1 so a missing daemon fails the job.
    if [ "${OPENROWS_REQUIRE_DOCKER:-0}" = 1 ]; then
        echo "FAIL: Docker is required (OPENROWS_REQUIRE_DOCKER=1) but not available" >&2
        exit 1
    fi
    echo "SKIP: Docker is not available; MySQL/MariaDB integration tests not run"
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
prefix="${OPENROWS_TEST_CONTAINER_PREFIX:-openrows}-mysql-test-$$"
network="$prefix-net"
tls_dir="$(mktemp -d "$PWD/.mysql-test-tls.XXXXXX")"
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
# The servers read these as their own unprivileged user.
chmod 755 "$tls_dir"
build_sshd
status=0

ping_server() {
    docker exec "$1" sh -c "
        admin=\$(command -v mariadb-admin || command -v mysqladmin)
        \"\$admin\" ping --host=127.0.0.1 --user=root --password=$password --silent" >/dev/null 2>&1
}

# Monitor digests need performance_schema (MariaDB defaults to OFF); set
# OPENROWS_TEST_MYSQL_PERFORMANCE_SCHEMA=OFF to exercise the disabled path.
start_mysql() {
    local name="$1" image="$2"
    docker run --detach --name "$name" --network "$network" --network-alias database \
        --publish 127.0.0.1::3306 --volume "$tls_dir:/certs:ro" \
        --env MYSQL_ROOT_PASSWORD="$password" --env MARIADB_ROOT_PASSWORD="$password" \
        --env MYSQL_DATABASE=openrows_test --env MARIADB_DATABASE=openrows_test \
        "$image" --performance-schema="${OPENROWS_TEST_MYSQL_PERFORMANCE_SCHEMA:-ON}" \
        --ssl-ca=/certs/ca.crt --ssl-cert=/certs/server.crt --ssl-key=/certs/server.key >/dev/null
    # The images start a socket-only server for initialisation first, so a
    # TCP ping only succeeds once the real server is up.
    local deadline=$((SECONDS + 180))
    while (( SECONDS < deadline )); do
        if ping_server "$name"; then
            return 0
        fi
    done
    ping_server "$name"
}

# The TiDB image has no shell or client, and its bootstrap file cannot set the
# root password; the MySQL image's client does that once the status port answers.
start_tidb() {
    local name="$1" image="$2"
    printf '[security]\nssl-ca = "/certs/ca.crt"\nssl-cert = "/certs/server.crt"\nssl-key = "/certs/server.key"\n' \
        >"$tls_dir/tidb.toml"
    docker run --detach --name "$name" --network "$network" --network-alias database \
        --publish 127.0.0.1::4000 --publish 127.0.0.1::10080 --volume "$tls_dir:/certs:ro" \
        "$image" --config=/certs/tidb.toml >/dev/null
    local status_port deadline=$((SECONDS + 180))
    status_port="$(docker port "$name" 10080/tcp | head -n 1 | sed 's/.*://')"
    until curl --fail --silent "http://127.0.0.1:$status_port/status" >/dev/null; do
        if (( SECONDS >= deadline )); then
            docker logs "$name" >&2
            return 1
        fi
    done
    docker run --rm --network "container:$name" mysql:8.4 mysql --host=127.0.0.1 --port=4000 \
        --user=root --execute="ALTER USER 'root'@'%' IDENTIFIED BY '$password'; FLUSH PRIVILEGES;
            CREATE DATABASE openrows_test" >/dev/null
}

# Tests of what TiDB does not have or does differently (docs/05 §13.1): stored
# routines and triggers, EXPLAIN FORMAT=JSON, performance_schema, ALTER TABLE changes
# that use a column added in the same statement, interruptible SLEEP, read-only
# transactions (the AI read path refuses to run without them), table grants in
# information_schema for an unprivileged login, and server tools in the container.
tidb_excluded='test(/procedures_return_several|views_triggers_routines|inspect_views_routines|alter_table_keeps|tables_keys_and_copies|explain_and_analyze_parse|activity_blockers_rates|cancel_kills_the_running|round_trip_with_the_containers_tools|mysql_refuses_attacks|unreadable_memberships/)'

# Run the suites on image $1 as engine $2, whose SQL port is $3.
run_suites() {
    local image="$1" engine="$2" sql_port="$3" name="$prefix-$2" port filter='all()'
    [ "$engine" != tidb ] || filter="not $tidb_excluded"
    docker network create "$network" >/dev/null
    if [ "$engine" = tidb ]; then
        start_tidb "$name" "$image"
    else
        start_mysql "$name" "$image"
    fi
    start_sshd "$prefix-sshd" "$network"
    export_routes "$prefix-sshd" database "$sql_port" "$tls_dir"
    port="$(docker port "$name" "$sql_port/tcp" | head -n 1 | sed 's/.*://')"
    echo "$image on 127.0.0.1:${port}, TLS on"
    OPENROWS_TEST_MYSQL_PORT="$port" OPENROWS_TEST_MYSQL_PASSWORD="$password" \
        OPENROWS_TEST_MYSQL_ENGINE="$engine" OPENROWS_TEST_MYSQL_CONTAINER="$name" \
        cargo nextest run -p openrows-driver-mysql -p openrows-core -p openrows-schema \
        -p openrows-mcp -p openrows-driver-conformance -p openrows-import -p openrows-dump --locked --test mysql \
        --run-ignored only --no-capture --no-fail-fast -E "$filter"
}

for image in "$@"; do
    case "$image" in
        mariadb*) engine=mariadb sql_port=3306 ;;
        mysql*) engine=mysql sql_port=3306 ;;
        *tidb*) engine=tidb sql_port=4000 ;;
        *) echo "usage: just test-mysql [mysql:<tag>|mariadb:<tag>|pingcap/tidb:<tag> ...]" >&2; exit 2 ;;
    esac
    run_suites "$image" "$engine" "$sql_port" || status=1
    remove_containers
done
exit "$status"

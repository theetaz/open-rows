#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Run the ignored MySQL/MariaDB driver integration tests against throwaway
# containers, one image after another (default: mysql:8.4 then mariadb:11.4).
# Skips (exit 0) when Docker is unavailable (fails with OPENROWS_REQUIRE_DOCKER=1);
# always removes the containers.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
if [ "$#" -eq 0 ]; then
    set -- mysql:8.4 mariadb:11.4
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
# Synthetic credential for containers bound to 127.0.0.1 and removed on exit.
password="openrows-test-only"
names=()
cleanup() {
    for name in "${names[@]}"; do
        docker rm --force --volumes "$name" >/dev/null 2>&1 || true
    done
}
trap cleanup EXIT INT TERM

ping_server() {
    docker exec "$1" sh -c "
        admin=\$(command -v mariadb-admin || command -v mysqladmin)
        \"\$admin\" ping --host=127.0.0.1 --user=root --password=$password --silent" >/dev/null 2>&1
}

for image in "$@"; do
    case "$image" in
        mariadb*) engine=mariadb ;;
        mysql*) engine=mysql ;;
        *) echo "usage: just test-mysql [mysql:<tag>|mariadb:<tag> ...]" >&2; exit 2 ;;
    esac
    name="openrows-${engine}-test-$$"
    names+=("$name")
    docker run --detach --rm --name "$name" --publish 127.0.0.1::3306 \
        --env MYSQL_ROOT_PASSWORD="$password" --env MARIADB_ROOT_PASSWORD="$password" \
        --env MYSQL_DATABASE=openrows_test --env MARIADB_DATABASE=openrows_test \
        "$image" >/dev/null
    # The images start a socket-only server for initialisation first, so a
    # TCP ping only succeeds once the real server is up.
    for _ in $(seq 1 180); do
        if ping_server "$name"; then
            break
        fi
        sleep 1
    done
    ping_server "$name"
    port="$(docker port "$name" 3306/tcp | head -n 1 | sed 's/.*://')"
    echo "$image on 127.0.0.1:${port}"
    OPENROWS_TEST_MYSQL_PORT="$port" OPENROWS_TEST_MYSQL_PASSWORD="$password" \
        OPENROWS_TEST_MYSQL_ENGINE="$engine" \
        cargo nextest run -p openrows-driver-mysql -p openrows-core -p openrows-schema \
        -p openrows-driver-conformance --locked --test mysql \
        --run-ignored only --no-capture
    docker rm --force --volumes "$name" >/dev/null 2>&1 || true
done

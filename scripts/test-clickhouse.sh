#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# ClickHouse LTS HTTP(S), native Arrow streaming and capability conformance.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
image="${1:-clickhouse/clickhouse-server:25.3.9.72}"
docker info >/dev/null
name="openrows-clickhouse-test-$$"
fixture="$PWD/.clickhouse-test-$$"
mkdir "$fixture"
cleanup() {
    docker rm --force --volumes "$name" >/dev/null 2>&1 || true
    rm -rf "$fixture"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
bash scripts/test-pg-certs.sh "$fixture"
cat > "$fixture/tls.xml" <<'XML'
<clickhouse>
  <https_port>8443</https_port>
  <openSSL><server>
    <certificateFile>/test-certs/server.crt</certificateFile>
    <privateKeyFile>/test-certs/server.key</privateKeyFile>
    <verificationMode>none</verificationMode>
  </server></openSSL>
</clickhouse>
XML
# Monitor restricted-user fixtures need system-table grants to be enforced;
# OPENROWS_TEST_CLICKHOUSE_QUERY_LOG=OFF removes query_log to test that path.
query_log=""
if [[ "${OPENROWS_TEST_CLICKHOUSE_QUERY_LOG:-ON}" == OFF ]]; then
    query_log='<query_log remove="1"/>'
fi
cat > "$fixture/monitor.xml" <<XML
<clickhouse>
  <access_control_improvements>
    <select_from_system_db_requires_grant>true</select_from_system_db_requires_grant>
  </access_control_improvements>
  $query_log
</clickhouse>
XML
docker run --detach --rm --cpus 2 --memory 2g --name "$name" --publish 127.0.0.1::8123 --publish 127.0.0.1::8443 \
    --env CLICKHOUSE_PASSWORD=test-only --env CLICKHOUSE_DB=openrows_test --env CLICKHOUSE_DEFAULT_ACCESS_MANAGEMENT=1 \
    --mount "type=bind,src=$fixture,dst=/test-certs,readonly" \
    --mount "type=bind,src=$fixture/tls.xml,dst=/etc/clickhouse-server/config.d/tls.xml,readonly" \
    --mount "type=bind,src=$fixture/monitor.xml,dst=/etc/clickhouse-server/config.d/monitor.xml,readonly" \
    "$image" >/dev/null
export OPENROWS_TEST_CLICKHOUSE_PORT="$(docker port "$name" 8123/tcp | head -n 1 | sed 's/.*://')"
export OPENROWS_TEST_CLICKHOUSE_TLS_PORT="$(docker port "$name" 8443/tcp | head -n 1 | sed 's/.*://')"
# The image starts a localhost-only initialization server and then restarts it.
# Query the published HTTP port so readiness means the final server is running.
ready() {
    local result
    result="$(curl --fail --silent --max-time 1 \
        --header 'X-ClickHouse-User: default' --header 'X-ClickHouse-Key: test-only' \
        --header 'X-ClickHouse-Database: openrows_test' \
        --data-binary 'SELECT 1' "http://127.0.0.1:$OPENROWS_TEST_CLICKHOUSE_PORT/")" || return 1
    [ "$result" = 1 ]
}
deadline=$((SECONDS + 120))
while (( SECONDS < deadline )); do
    if ready; then break; fi
done
if ! ready; then
    docker logs "$name" >&2
    exit 1
fi
export OPENROWS_TEST_CLICKHOUSE_PASSWORD=test-only OPENROWS_TEST_CLICKHOUSE_CA="$fixture/ca.crt"
cargo nextest run -p openrows-driver-clickhouse --locked --test clickhouse --run-ignored only --success-output immediate --no-fail-fast
cargo nextest run -p openrows-driver-conformance --locked --test clickhouse --run-ignored only --test-threads 1 --no-fail-fast

# The ingest benchmark asserts docs/13 budgets; timing is only meaningful on an
# idle machine, so it runs on request: OPENROWS_BENCH=1 just test-clickhouse.
if [[ "${OPENROWS_BENCH:-}" == 1 ]]; then
    python3 -B scripts/bench-clickhouse.py
fi

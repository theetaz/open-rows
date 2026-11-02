# SPDX-License-Identifier: Apache-2.0
# Sourced by the container test scripts: the OpenSSH fixture of the openrows-net
# tests, on the database's Docker network, for the conformance SSH group.

# Synthetic credential of the fixture's `tester` user, removed with the container.
sshd_password="openrows-test-only"

# Build the fixture image once per script.
build_sshd() {
    docker build --quiet --tag openrows-test-sshd:m1-06 crates/openrows-net/tests/fixtures/sshd >/dev/null
}

# Start an sshd named $1 on network $2 and wait until it listens.
start_sshd() {
    docker run --detach --rm --name "$1" --network "$2" --publish 127.0.0.1::22 \
        --env TEST_PASSWORD="$sshd_password" --env AUTHORIZED_KEYS= \
        openrows-test-sshd:m1-06 >/dev/null
    local deadline=$((SECONDS + 30))
    while (( SECONDS < deadline )); do
        if docker logs "$1" 2>&1 | grep --quiet "Server listening"; then
            return 0
        fi
    done
    docker logs "$1" >&2
    return 1
}

# Export what the conformance SSH and TLS groups read (docs/17 §2.2): sshd
# container $1, the database alias and port $2 and $3 as it sees them, TLS files $4.
export_routes() {
    OPENROWS_TEST_SSH_PORT="$(docker port "$1" 22/tcp | head -n 1 | sed 's/.*://')"
    export OPENROWS_TEST_SSH_PORT OPENROWS_TEST_SSH_PASSWORD="$sshd_password" \
        OPENROWS_TEST_ROUTED_HOST="$2" OPENROWS_TEST_ROUTED_PORT="$3" OPENROWS_TEST_TLS_DIR="$4"
}

#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Generate throwaway TLS material for `just test-pg` into <dir>: a test CA, a
# server certificate for DNS name `localhost` only (so 127.0.0.1 exercises
# host-name mismatches), a client certificate for role `certuser`, an
# unrelated CA, and the init script that adds the TLS test roles.
set -euo pipefail
dir="${1:?usage: test-pg-certs.sh <dir>}"
cd "$dir"
quiet() { "$@" >/dev/null 2>&1; }
ec_key() { quiet openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 -pkeyopt ec_param_enc:named_curve -out "$1"; }
ca() {
    ec_key "$1.key"
    quiet openssl req -sha256 -x509 -new -key "$1.key" -subj "/CN=$2" -days 2 -out "$1.crt" \
        -addext basicConstraints=critical,CA:TRUE -addext keyUsage=critical,keyCertSign,cRLSign
}
leaf() {
    ec_key "$1.key"
    quiet openssl req -sha256 -new -key "$1.key" -subj "/CN=$2" -out "$1.csr"
    printf '%s\n' "$3" >"$1.ext"
    quiet openssl x509 -sha256 -req -in "$1.csr" -CA ca.crt -CAkey ca.key -CAcreateserial -days 2 \
        -extfile "$1.ext" -out "$1.crt"
    rm -f "$1.csr" "$1.ext"
}
ca ca "OpenRows Test CA"
ca other-ca "OpenRows Unrelated CA"
leaf server localhost "subjectAltName=DNS:localhost
extendedKeyUsage=serverAuth"
leaf client certuser "extendedKeyUsage=clientAuth"
# Keys are readable only by the owner; the container copies the server key.
chmod 600 ./*.key
chmod 644 server.key
cat >init-tls.sh <<'SQL'
#!/bin/sh
# Roles for the TLS tests: certificate authentication, and a role that may
# only connect over TLS (exercises sslmode=allow's retry).
set -e
psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" \
    -c "CREATE ROLE certuser LOGIN" \
    -c "CREATE ROLE tlsonly LOGIN PASSWORD '$POSTGRES_PASSWORD'"
{
    printf 'hostssl all certuser all cert\n'
    printf 'hostnossl all tlsonly all reject\n'
    cat "$PGDATA/pg_hba.conf"
} >"$PGDATA/pg_hba.conf.openrows"
mv "$PGDATA/pg_hba.conf.openrows" "$PGDATA/pg_hba.conf"
SQL
chmod 755 init-tls.sh

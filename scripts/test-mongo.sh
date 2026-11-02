#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# MongoDB 8 replica set: atomic document edits and native conformance.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
docker info >/dev/null
scratch="$(mktemp -d "$PWD/.mongo-test.XXXXXX")"
name="openrows-mongodb-test-$$"
cleanup() { docker rm --force --volumes "$name" >/dev/null 2>&1 || true; rm -rf "$scratch"; }
trap cleanup EXIT INT TERM
openssl_bin="${OPENROWS_OPENSSL:-openssl}"
if [[ -x /opt/homebrew/opt/openssl@3/bin/openssl ]]; then openssl_bin=/opt/homebrew/opt/openssl@3/bin/openssl; fi
"$openssl_bin" req -x509 -newkey rsa:2048 -nodes -days 1 -subj /CN=localhost \
    -addext 'subjectAltName=DNS:localhost,IP:127.0.0.1' \
    -keyout "$scratch/ca-key.pem" -out "$scratch/ca.pem" >/dev/null 2>&1
"$openssl_bin" req -new -newkey rsa:2048 -nodes -subj /CN=localhost \
    -keyout "$scratch/key.pem" -out "$scratch/server.csr" >/dev/null 2>&1
cat > "$scratch/server.ext" <<'EOF'
basicConstraints=critical,CA:FALSE
keyUsage=critical,digitalSignature,keyEncipherment
extendedKeyUsage=serverAuth,clientAuth
subjectAltName=DNS:localhost,IP:127.0.0.1
EOF
"$openssl_bin" x509 -req -in "$scratch/server.csr" -CA "$scratch/ca.pem" \
    -CAkey "$scratch/ca-key.pem" -CAcreateserial -days 1 \
    -extfile "$scratch/server.ext" -out "$scratch/server-cert.pem" >/dev/null 2>&1
cat "$scratch/key.pem" "$scratch/server-cert.pem" > "$scratch/server.pem"
"$openssl_bin" verify -purpose sslserver -CAfile "$scratch/ca.pem" "$scratch/server-cert.pem" >/dev/null
"$openssl_bin" verify -purpose sslclient -CAfile "$scratch/ca.pem" "$scratch/server-cert.pem" >/dev/null
chmod 644 "$scratch"/*.pem
docker run --detach --rm --name "$name" --cpus 2 --memory 2g \
    --publish 127.0.0.1::27017 --volume "$scratch:/certs:ro" mongo:8 --replSet openrows --bind_ip_all --setParameter enableTestCommands=1 \
    --tlsMode preferTLS --tlsCertificateKeyFile /certs/server.pem --tlsCAFile /certs/ca.pem --tlsAllowConnectionsWithoutCertificates >/dev/null
port="$(docker port "$name" 27017/tcp | head -n 1 | sed 's/.*://')"
deadline=$((SECONDS + 90))
until docker exec "$name" mongosh --quiet --eval 'db.adminCommand({ping:1}).ok' >/dev/null 2>&1; do
    if (( SECONDS >= deadline )); then docker logs "$name"; exit 1; fi
done
docker exec "$name" mongosh --quiet --eval 'rs.initiate({_id:"openrows",members:[{_id:0,host:"localhost:27017"}]})' >/dev/null
until docker exec "$name" mongosh --quiet --eval 'if (!db.hello().isWritablePrimary) quit(1)' >/dev/null 2>&1; do
    if (( SECONDS >= deadline )); then docker logs "$name"; exit 1; fi
done
export OPENROWS_TEST_MONGO_PORT="$port" OPENROWS_TEST_MONGO_CA="$scratch/ca.pem"
cargo nextest run -p openrows-driver-mongodb --locked --test mongo --run-ignored only --test-threads 1
cargo nextest run -p openrows-driver-conformance --locked --test mongodb --run-ignored only --test-threads 1

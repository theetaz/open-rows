#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# One-time, opt-in: create a self-signed code-signing identity ("OpenRows Dev") in the login
# keychain so `just dev` can sign debug builds with a stable signature. The keychain then keeps
# "Always Allow" for OpenRows' saved passwords across rebuilds (see scripts/dev-sign-run.sh).
#
# The certificate is only for local debug builds: it is not trusted system-wide, never used
# for releases (those use Developer ID signing), and can be removed in Keychain Access.
#
# OPENROWS_DEV_IDENTITY  identity name (default "OpenRows Dev").
# OPENROWS_DEV_KEYCHAIN  keychain file (default: the login keychain).
set -euo pipefail

name=${OPENROWS_DEV_IDENTITY:-OpenRows Dev}
keychain=${OPENROWS_DEV_KEYCHAIN:-$HOME/Library/Keychains/login.keychain-db}
# LibreSSL writes PKCS#12 files `security import` reads; OpenSSL 3 would need -legacy.
openssl=/usr/bin/openssl

if [[ "$(uname -s)" != Darwin ]]; then
    echo "dev-cert: macOS only; other platforms need no dev signing identity."
    exit 0
fi
identities=$(security find-identity -p codesigning "$keychain")
if grep -qF "\"$name\"" <<<"$identities"; then
    echo "dev-cert: \"$name\" already exists in $keychain; nothing to do."
    exit 0
fi

work=$(mktemp -d)
cleanup() { rm -rf "$work"; }
trap cleanup EXIT

cat >"$work/openssl.cnf" <<'EOF'
[req]
distinguished_name = dn
[dn]
[codesign]
basicConstraints = critical, CA:false
keyUsage = critical, digitalSignature
extendedKeyUsage = critical, codeSigning
subjectKeyIdentifier = hash
EOF

# Throwaway passphrase protecting the PKCS#12 file for the moment it exists on disk.
passphrase=$("$openssl" rand -hex 24)
"$openssl" req -x509 -newkey rsa:2048 -nodes -days 3650 -subj "/CN=$name" \
    -config "$work/openssl.cnf" -extensions codesign \
    -keyout "$work/key.pem" -out "$work/cert.pem" 2>/dev/null
"$openssl" pkcs12 -export -name "$name" -inkey "$work/key.pem" -in "$work/cert.pem" \
    -passout "pass:$passphrase" -out "$work/identity.p12"
# -T lets codesign use the private key; no other application gets access.
security import "$work/identity.p12" -k "$keychain" -f pkcs12 -P "$passphrase" \
    -T /usr/bin/codesign >/dev/null

echo "dev-cert: created \"$name\" in $keychain."

# Sign a scratch copy of /usr/bin/true now, so the one-time key-access prompt appears here
# rather than in the middle of `just dev`.
echo "dev-cert: test-signing; if macOS asks to let codesign use the key, enter your login"
echo "dev-cert: password and choose Always Allow."
cp /usr/bin/true "$work/probe"
if ! codesign --force --timestamp=none --keychain "$keychain" --sign "$name" "$work/probe" 2>/dev/null; then
    echo "dev-cert: test signing failed; 'just dev' will ask again when it signs." >&2
fi
echo "dev-cert: done. 'just dev' now signs target/debug/openrows-app with it. On the first"
echo "dev-cert: connect after that, macOS asks once more for the saved password item: choose"
echo "dev-cert: Always Allow; later rebuilds keep that permission."

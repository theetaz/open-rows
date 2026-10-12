#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Source this in the runner build step; all temporary credentials are destroyed on exit.
set -euo pipefail
credential_dir=$(mktemp -d "${RUNNER_TEMP:?}/openrows-signing.XXXXXX")
keychain="$credential_dir/release.keychain-db"
cleanup_credentials() {
    local status=0
    if [[ -f "$keychain" ]]; then security delete-keychain "$keychain" || status=$?; fi
    rm -rf "$credential_dir"
    return "$status"
}
trap cleanup_credentials EXIT
if [[ -n "${APPLE_CERTIFICATE:-}" ]]; then
    : "${APPLE_CERTIFICATE_PASSWORD?Certificate password required}"
    : "${APPLE_SIGNING_IDENTITY:?Developer ID identity required}"
    keychain_password=$(openssl rand -hex 32)
    printf '%s' "$APPLE_CERTIFICATE" | base64 --decode > "$credential_dir/certificate.p12"
    security create-keychain -p "$keychain_password" "$keychain"
    security unlock-keychain -p "$keychain_password" "$keychain"
    security set-keychain-settings -lut 21600 "$keychain"
    security import "$credential_dir/certificate.p12" -k "$keychain" -P "$APPLE_CERTIFICATE_PASSWORD" -T /usr/bin/codesign
    security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$keychain_password" "$keychain" >/dev/null
    security list-keychains -d user -s "$keychain" login.keychain-db
    rm "$credential_dir/certificate.p12"
fi
if [[ -n "${APPLE_API_PRIVATE_KEY:-}" ]]; then
    export APPLE_API_KEY_PATH="$credential_dir/AuthKey.p8"
    printf '%s' "$APPLE_API_PRIVATE_KEY" > "$APPLE_API_KEY_PATH"
    chmod 600 "$APPLE_API_KEY_PATH"
fi

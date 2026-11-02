#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
set -euo pipefail
[[ -n "${APPLE_SIGNING_IDENTITY:-}" && "$APPLE_SIGNING_IDENTITY" != - ]] || exit 0
if [[ -n "${APPLE_API_KEY_PATH:-}" ]]; then
    credentials=(--key "$APPLE_API_KEY_PATH" --key-id "${APPLE_API_KEY:?}" --issuer "${APPLE_API_ISSUER:?}")
else
    credentials=(--apple-id "${APPLE_ID:?}" --team-id "${APPLE_TEAM_ID:?}" --password "${APPLE_PASSWORD:?}")
fi
for dmg in "$@"; do
    codesign --force --timestamp --sign "$APPLE_SIGNING_IDENTITY" "$dmg"
    result=$(xcrun notarytool submit "$dmg" "${credentials[@]}" --wait --output-format json)
    printf '%s' "$result" | python3 -c 'import json, sys; result=json.load(sys.stdin); assert result["status"] == "Accepted", result["status"]'
    xcrun stapler staple "$dmg"
    xcrun stapler validate "$dmg"
done

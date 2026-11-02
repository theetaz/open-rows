#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Decide which release stages have complete credentials. Only variable names are printed.
set -euo pipefail

missing() {
    local name
    for name in "$@"; do
        [[ -n "${!name:-}" ]] || printf ' %s' "$name"
    done
}

any_set() {
    local name
    for name in "$@"; do
        [[ -z "${!name:-}" ]] || return 0
    done
    return 1
}

fail() {
    printf '::error title=Incomplete release credentials::%s\n' "$1" >&2
    exit 1
}

certificate=(APPLE_CERTIFICATE APPLE_CERTIFICATE_PASSWORD APPLE_SIGNING_IDENTITY)
api=(APPLE_API_ISSUER APPLE_API_KEY APPLE_API_PRIVATE_KEY)
account=(APPLE_ID APPLE_TEAM_ID APPLE_PASSWORD)
updater=(TAURI_SIGNING_PRIVATE_KEY TAURI_SIGNING_PUBLIC_KEY)

apple=false
if any_set "${certificate[@]}" "${api[@]}" "${account[@]}"; then
    [[ -z "$(missing "${certificate[@]}")" ]] || fail "Developer ID signing is missing:$(missing "${certificate[@]}")"
    [[ "$APPLE_SIGNING_IDENTITY" != - ]] || fail "APPLE_SIGNING_IDENTITY must be a Developer ID identity, not ad-hoc (-)"
    if any_set "${api[@]}"; then
        [[ -z "$(missing "${api[@]}")" ]] || fail "App Store Connect notarisation is missing:$(missing "${api[@]}")"
    else
        [[ -z "$(missing "${account[@]}")" ]] || fail "Apple ID notarisation is missing:$(missing "${account[@]}")"
    fi
    apple=true
else
    printf '::notice title=Signing skipped::Apple Developer ID secrets are not configured; building an ad-hoc signed DMG without notarisation.\n'
fi

update=false
if any_set "${updater[@]}"; then
    [[ -z "$(missing "${updater[@]}")" ]] || fail "Updater signing is missing:$(missing "${updater[@]}")"
    update=true
else
    printf '::notice title=Updater skipped::TAURI_SIGNING_PRIVATE_KEY and TAURI_SIGNING_PUBLIC_KEY are not configured; no update archives or feed.\n'
fi

publish=false
if [[ "$apple" == true && "$update" == true ]]; then
    publish=true
else
    printf '::notice title=Publication skipped::A GitHub Release needs both Apple and updater credentials; DMGs stay workflow artifacts only.\n'
fi
printf 'apple=%s\nupdater=%s\npublish=%s\n' "$apple" "$update" "$publish" | tee -a "${GITHUB_OUTPUT:-/dev/null}"

#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Cargo target runner for `just dev` on macOS: re-sign the debug openrows-app with the stable
# local identity from `just dev-cert`, then run it. Every other binary runs untouched.
#
# Debug builds are ad-hoc signed, so each rebuild changes the code signature and the keychain
# forgets "Always Allow". A stable identity keeps the signature's designated requirement, so
# the keychain keeps trusting rebuilds. Without the identity this prints a hint and only runs.
#
# OPENROWS_DEV_IDENTITY  signing identity name (default "OpenRows Dev").
# OPENROWS_DEV_SIGN_DRY_RUN=1  print the codesign command instead of running it.
# See CONTRIBUTING.md (Development) and docs/12 §3.
set -euo pipefail

identity_name=${OPENROWS_DEV_IDENTITY:-OpenRows Dev}
signing_id=com.prabhavalabs.openrows.dev

# SHA-1 of the first code-signing identity called $identity_name, if any.
identity_hash() {
    local identities
    identities=$(security find-identity -p codesigning 2>/dev/null) || return 0
    awk -v name="\"$identity_name\"" 'index($0, name) { print $2; exit }' <<<"$identities"
}

# True when the binary already carries a signature from the identity.
signed_by_identity() {
    local details
    details=$(codesign -dvv "$1" 2>&1) || return 1
    grep -qxF "Authority=$identity_name" <<<"$details"
}

sign() {
    local binary=$1 hash
    hash=$(identity_hash)
    if [[ -z "$hash" ]]; then
        printf 'dev-sign: no "%s" identity; keychain prompts return after rebuilds (run "just dev-cert" once).\n' \
            "$identity_name" >&2
        return 0
    fi
    signed_by_identity "$binary" && return 0
    local command=(codesign --force --timestamp=none --identifier "$signing_id" --sign "$hash" "$binary")
    if [[ "${OPENROWS_DEV_SIGN_DRY_RUN:-}" == 1 ]]; then
        printf 'dev-sign: dry run:' >&2
        printf ' %q' "${command[@]}" >&2
        printf '\n' >&2
        return 0
    fi
    local output
    if ! output=$("${command[@]}" 2>&1); then
        printf 'dev-sign: codesign failed (%s); running the ad-hoc build.\n' "$output" >&2
    fi
}

if [[ $# -eq 0 ]]; then
    printf 'usage: %s <binary> [args...]\n' "$0" >&2
    exit 2
fi
if [[ "$(uname -s)" == Darwin && "$(basename -- "$1")" == openrows-app ]]; then
    sign "$1"
fi
exec "$@"

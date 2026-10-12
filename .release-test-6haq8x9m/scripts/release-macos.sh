#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Local mode never imports certificates or contacts Apple's notarization service.
set -euo pipefail
cd "$(dirname "$0")/.."
mode=${1:-local}
target=${2:-$(rustc -vV | sed -n 's/^host: //p')}
case "$target" in aarch64-apple-darwin|x86_64-apple-darwin) ;; *) exit 2 ;; esac
export CARGO_BUILD_TARGET="$target" CI=true MACOSX_DEPLOYMENT_TARGET=12.0
output="${CARGO_TARGET_DIR:-$PWD/target}/$target/release/bundle"
options=(--target "$target" --bundles "app,dmg")
# GitHub injects unset secrets as empty strings; Tauri expects absent variables.
for variable in APPLE_SIGNING_IDENTITY APPLE_ID APPLE_TEAM_ID APPLE_PASSWORD APPLE_API_ISSUER APPLE_API_KEY APPLE_API_KEY_PATH; do
    [[ -n "${!variable:-}" ]] || unset "$variable"
done
case "$mode" in
    local)
        unset APPLE_CERTIFICATE APPLE_CERTIFICATE_PASSWORD APPLE_SIGNING_IDENTITY
        unset APPLE_ID APPLE_TEAM_ID APPLE_PASSWORD APPLE_API_ISSUER APPLE_API_KEY APPLE_API_KEY_PATH APPLE_API_PRIVATE_KEY
        unset TAURI_SIGNING_PRIVATE_KEY TAURI_SIGNING_PRIVATE_KEY_PASSWORD
        unset VITE_OPENROWS_UPDATE_KEY VITE_OPENROWS_UPDATE_STABLE VITE_OPENROWS_UPDATE_BETA
        ;;
    release)
        if [[ -n "${TAURI_SIGNING_PRIVATE_KEY:-}" ]]; then
            config=$(python3 -B scripts/release-config.py)
            options+=(--config "$config")
            export VITE_OPENROWS_UPDATE_KEY="$TAURI_SIGNING_PUBLIC_KEY"
            export VITE_OPENROWS_UPDATE_STABLE="https://github.com/$GITHUB_REPOSITORY/releases/latest/download/stable.json"
            export VITE_OPENROWS_UPDATE_BETA="https://github.com/$GITHUB_REPOSITORY/releases/download/update-beta/beta.json"
        fi
        ;;
    *) printf 'Usage: release-macos.sh local|release [target]\n' >&2; exit 2 ;;
esac
bash scripts/sccache-guard.sh pnpm --filter @openrows/desktop tauri build "${options[@]}"
python3 -B scripts/check-installer-size.py "$output"/dmg/*.dmg
codesign --verify --deep --strict "$output/macos/OpenRows.app"
codesign --verify --strict "$output/macos/OpenRows.app/Contents/MacOS/openrows-driver-host"

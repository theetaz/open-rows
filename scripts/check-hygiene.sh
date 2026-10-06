#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Check tracked and new, non-ignored sources, including paths with spaces.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
failed=0
while IFS= read -r -d '' path; do
    case "$path" in
        *.rs|*.ts|*.tsx) ;;
        *) continue ;;
    esac
    [[ -f "$path" ]] || continue
    generated=0
    case "/$path" in
        */generated/*) generated=1 ;;
    esac
    if ! awk -v generated="$generated" '
        {
            rest = $0
            while (match(rest, /TODO|FIXME/)) {
                marker = substr(rest, RSTART)
                if (marker !~ /^(TODO|FIXME)\(#[0-9]+\)/) {
                    printf "%s:%d: TODO/FIXME requires an issue reference, e.g. TODO(#123)\n", FILENAME, FNR
                    failed = 1
                }
                rest = substr(rest, RSTART + RLENGTH)
            }
        }
        END {
            if (!generated && FNR > 600) {
                printf "%s: %d lines exceeds hard limit of 600\n", FILENAME, FNR
                failed = 1
            }
            exit failed
        }
    ' "$path"; then
        failed=1
    fi
done < <(git ls-files --cached --others --exclude-standard -z)
exit "$failed"

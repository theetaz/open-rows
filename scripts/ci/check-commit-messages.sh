#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Validate non-merge commits using the same rules as local commits.
set -euo pipefail

if [[ $# != 2 ]]; then
    printf 'Usage: %s BASE_SHA HEAD_SHA\n' "$0" >&2
    exit 2
fi
hook="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../git-hooks" && pwd)/commit-msg"
base=$(git rev-parse --verify --end-of-options "${1}^{commit}")
head=$(git rev-parse --verify --end-of-options "${2}^{commit}")
commits=$(git rev-list --reverse --no-merges "$base..$head")
message=$(mktemp "${TMPDIR:-/tmp}/openrows-commit-message.XXXXXX")
trap 'rm -f -- "$message"' EXIT
count=0
while IFS= read -r commit; do
    [[ -n "$commit" ]] || continue
    git show -s --format=%B "$commit" > "$message"
    if ! bash "$hook" "$message"; then
        printf 'Invalid commit message: %s\n' "$commit" >&2
        exit 1
    fi
    count=$((count + 1))
done <<< "$commits"
printf 'Checked %d non-merge commit message(s).\n' "$count"

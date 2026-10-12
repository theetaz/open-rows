#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
set -euo pipefail
pnpm build
# Only the executable is inspected here; the production helper can be debug.
# A missing staging prerequisite must still fail rather than skipping the audit.
bash scripts/stage-host.sh dev
cargo build -p openrows-app --release --features test-automation,tauri/custom-protocol --locked
binary="${CARGO_TARGET_DIR:-target}/${CARGO_BUILD_TARGET:+$CARGO_BUILD_TARGET/}release/openrows-app"
python3 -B - "$binary" <<'PY'
import pathlib
import sys
binary = pathlib.Path(sys.argv[1]).read_bytes()
for marker in (b'automation_reply', b'automation_observations', b'OPENROWS_E2E_ROOT', b'bridge.sock'):
    if marker in binary:
        raise SystemExit(f'Release includes automation marker: {marker!r}')
print('PASS: release build with test-automation feature excludes all bridge markers')
PY

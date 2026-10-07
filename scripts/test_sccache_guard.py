# SPDX-License-Identifier: Apache-2.0
"""Tests for sccache-guard.sh using a stub sccache; no real server is started."""

import os
import subprocess
import tempfile
import unittest
from pathlib import Path

GUARD = Path(__file__).resolve().parent / "sccache-guard.sh"

# Modes: healthy probes pass; stale probes fail until a restart; blocked probes always fail.
STUB = """#!/usr/bin/env bash
printf '%s\\n' "$1" >>"$STUB_LOG"
case "$1" in
    --stop-server) exit 0 ;;
    --start-server) [[ "$STUB_MODE" == blocked ]] && exit 2; : >"$STUB_RESTARTED"; exit 0 ;;
esac
if [[ "$STUB_MODE" == blocked || ("$STUB_MODE" == stale && ! -e "$STUB_RESTARTED") ]]; then
    printf 'sccache: error: Operation not permitted (os error 1)\\n' >&2
    exit 254
fi
"""

SHOW_WRAPPER = ["bash", "-c", 'printf "%s" "${RUSTC_WRAPPER-unset}"; exit 7']


class SccacheGuardTest(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = Path(tempfile.mkdtemp())
        self.addCleanup(subprocess.run, ["rm", "-rf", str(self.tmp)], check=True)
        bin_dir = self.tmp / "bin"
        bin_dir.mkdir()
        self.stub = bin_dir / "sccache"
        self.stub.write_text(STUB)
        self.stub.chmod(0o755)
        self.log = self.tmp / "calls.log"
        self.target = self.tmp / "target"

    def run_guard(self, mode: str, wrapper: str | None) -> subprocess.CompletedProcess[str]:
        env = {
            "PATH": os.environ["PATH"],
            "CARGO_TARGET_DIR": str(self.target),
            "STUB_MODE": mode,
            "STUB_LOG": str(self.log),
            "STUB_RESTARTED": str(self.tmp / "restarted"),
        }
        if wrapper is not None:
            env["RUSTC_WRAPPER"] = wrapper
        return subprocess.run(
            ["bash", str(GUARD), *SHOW_WRAPPER],
            env=env,
            capture_output=True,
            text=True,
            stdin=subprocess.DEVNULL,
        )

    def calls(self) -> list[str]:
        return self.log.read_text().splitlines() if self.log.exists() else []

    def test_unset_wrapper_runs_command_untouched(self) -> None:
        result = self.run_guard("blocked", None)
        self.assertEqual((result.returncode, result.stdout), (7, "unset"))
        self.assertEqual(self.calls(), [])

    def test_other_wrapper_is_not_probed(self) -> None:
        result = self.run_guard("blocked", "/usr/bin/env")
        self.assertEqual(result.stdout, "/usr/bin/env")
        self.assertEqual(self.calls(), [])

    def test_healthy_server_keeps_wrapper_without_restart(self) -> None:
        result = self.run_guard("healthy", str(self.stub))
        self.assertEqual((result.returncode, result.stdout), (7, str(self.stub)))
        self.assertEqual(self.calls(), ["rustc"])
        self.assertEqual(result.stderr, "")

    def test_stale_server_is_restarted_and_wrapper_kept(self) -> None:
        result = self.run_guard("stale", str(self.stub))
        self.assertEqual(result.stdout, str(self.stub))
        self.assertEqual(self.calls(), ["rustc", "--stop-server", "--start-server", "rustc"])
        self.assertIn("restarting it in this context", result.stderr)

    def test_unreachable_server_falls_back_to_no_wrapper(self) -> None:
        result = self.run_guard("blocked", str(self.stub))
        self.assertEqual((result.returncode, result.stdout), (7, ""))
        self.assertIn("building without RUSTC_WRAPPER", result.stderr)

    def test_probe_leaves_no_files_in_target(self) -> None:
        self.run_guard("stale", str(self.stub))
        self.assertEqual(list(self.target.iterdir()), [])


if __name__ == "__main__":
    unittest.main()

# SPDX-License-Identifier: Apache-2.0
"""Exercise real staging with a stub compiler and protect Cargo's test helper."""

import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
HOST = "aarch64-apple-darwin"
CARGO = """
import json
import os
import sys
from pathlib import Path

args = sys.argv[1:]
root = Path(os.environ["CARGO_TARGET_DIR"])
profile = args[args.index("--profile") + 1]
profile = "debug" if profile == "dev" else profile
target = args[args.index("--target") + 1] if "--target" in args else ""
suffix = ".exe" if "-windows-" in target else ""
binary = root / target / profile / ("openrows-driver-host" + suffix)
binary.parent.mkdir(parents=True, exist_ok=True)
binary.write_bytes(b"production registry")
Path(os.environ["STUB_LOG"]).write_text(json.dumps({
    "args": args,
    "wrapper": os.environ.get("RUSTC_WRAPPER"),
    "jobs": os.environ.get("CARGO_BUILD_JOBS"),
}))
"""


class StageHostTest(unittest.TestCase):
    def setUp(self) -> None:
        directory = tempfile.TemporaryDirectory(prefix=".stage-host-test-", dir=ROOT)
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        scripts = self.root / "scripts"
        scripts.mkdir()
        for name in ("stage-host.sh", "sccache-guard.sh"):
            shutil.copyfile(ROOT / "scripts" / name, scripts / name)
        self.bin = self.root / "bin"
        self.bin.mkdir()
        self.write_stub("rustc", f"#!/usr/bin/env bash\nprintf 'host: {HOST}\\n'\n")
        self.write_stub("cargo", f"#!{sys.executable}\n{CARGO}")
        self.cache = self.root / "cache"
        self.log = self.root / "calls.json"

    def write_stub(self, name: str, content: str) -> None:
        path = self.bin / name
        path.write_text(content)
        path.chmod(0o755)

    def check_staging(self, profile: str, target: str, env_target: str = "") -> None:
        selected = target or env_target or HOST
        folder = "debug" if profile == "dev" else profile
        suffix = ".exe" if "-windows-" in selected else ""
        test_helper = self.cache / folder / f"openrows-driver-host{suffix}"
        test_helper.parent.mkdir(parents=True, exist_ok=True)
        test_helper.write_bytes(b"test sqlite registry")
        fixture = test_helper.with_name(f"host-test-fixture{suffix}")
        fixture.write_bytes(b"test sqlite registry")
        env = {
            **os.environ,
            "PATH": f"{self.bin}{os.pathsep}{os.environ['PATH']}",
            "CARGO_TARGET_DIR": str(self.cache),
            "CARGO_BUILD_TARGET": env_target,
            "RUSTC_WRAPPER": "test-wrapper",
            "CARGO_BUILD_JOBS": "4",
            "STUB_LOG": str(self.log),
        }
        result = subprocess.run(
            ["bash", "scripts/stage-host.sh", profile, target],
            cwd=self.root, env=env, capture_output=True, text=True, check=True,
        )
        self.assertIn("stage-host: production", result.stdout)
        self.check_output(test_helper, selected, folder, suffix)

    def check_output(self, test_helper: Path, target: str, folder: str, suffix: str) -> None:
        staged = self.root / "apps/desktop/src-tauri/binaries" / (
            f"openrows-driver-host-{target}{suffix}"
        )
        self.assertEqual(staged.read_bytes(), b"production registry")
        self.assertEqual(test_helper.read_bytes(), b"test sqlite registry")
        self.assertFalse(staged.samefile(test_helper))
        fixture = test_helper.with_name(f"host-test-fixture{suffix}")
        self.assertEqual(fixture.read_bytes(), b"test sqlite registry")
        self.assertFalse(staged.samefile(fixture))
        built = self.cache / target / folder / f"openrows-driver-host{suffix}"
        self.assertEqual(built.read_bytes(), staged.read_bytes())
        call = json.loads(self.log.read_text())
        self.assertIn("--no-default-features", call["args"])
        self.assertNotIn("--features", call["args"])
        self.assertEqual(call["args"][-2:], ["--target", target])
        self.assertEqual((call["wrapper"], call["jobs"]), ("test-wrapper", "4"))

    def test_native_debug_staging_preserves_test_helper(self) -> None:
        self.check_staging("dev", "")

    def test_native_release_staging_preserves_bench_helper(self) -> None:
        self.check_staging("release", "")

    def test_custom_profile_staging_preserves_test_helper(self) -> None:
        self.check_staging("packaging", "")

    def test_environment_target_is_used(self) -> None:
        self.check_staging("dev", "", "x86_64-unknown-linux-gnu")

    def test_explicit_windows_target_overrides_environment(self) -> None:
        self.check_staging("dev", "x86_64-pc-windows-msvc", HOST)


if __name__ == "__main__":
    unittest.main()

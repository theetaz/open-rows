# SPDX-License-Identifier: Apache-2.0
"""Protect packaging order, build-cache resolution and development images."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parent.parent
SPEC = importlib.util.spec_from_file_location("strip_bundle", ROOT / "scripts/strip-macos-bundle.py")
BUNDLE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BUNDLE)
TRIPLE = "aarch64-apple-darwin"


class StripBundleTest(unittest.TestCase):
    def test_non_macos_and_debug_do_not_run_tools(self):
        for environment in ({"TAURI_ENV_PLATFORM": "linux"},
                            {"TAURI_ENV_PLATFORM": "darwin", "TAURI_ENV_DEBUG": "true"}):
            with patch.dict(os.environ, environment, clear=True), patch.object(subprocess, "run") as run:
                BUNDLE.main()
                run.assert_not_called()

    def test_symbols_are_saved_once_and_only_release_images_are_stripped(self):
        with tempfile.TemporaryDirectory(prefix=".strip-test-", dir=ROOT) as directory:
            self.check_release(Path(directory))

    def check_release(self, root):
        target = root / "cache"
        apps = [target / "release/openrows-app", target / TRIPLE / "release/openrows-app"]
        helper = root / "apps/desktop/src-tauri/binaries" / f"openrows-driver-host-{TRIPLE}"
        debug = target / "debug/openrows-app"
        for path in [*apps, helper, debug]:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b"symbols and code")
        calls = []
        environment = {"TAURI_ENV_PLATFORM": "darwin", "TAURI_ENV_TARGET_TRIPLE": TRIPLE}
        with patch.object(BUNDLE, "ROOT", root), patch.dict(os.environ, environment, clear=True), \
             patch.object(subprocess, "run", side_effect=lambda cmd, **kw: self.tool(cmd, target, calls)):
            BUNDLE.main()
            BUNDLE.main()
        self.assertEqual(debug.read_bytes(), b"symbols and code")
        self.assertEqual([cmd[1] for cmd in calls if cmd[0] == "strip"],
                         [str(path) for path in [*apps, helper]] * 2)
        self.assertEqual(sum(cmd[0] == "dsymutil" for cmd in calls), 3)
        images = list((target / "bundle-symbols").glob("*.macho"))
        self.assertEqual(len(images), 3)
        self.assertTrue(all(image.read_bytes() == b"symbols and code" for image in images))
        for index, cmd in enumerate(calls):
            if cmd[0] == "dsymutil":
                self.assertEqual(calls[index + 1], ["strip", cmd[1]])
                self.assertEqual(calls[index + 2], ["codesign", "--force", "--sign", "-", cmd[1]])

    def tool(self, command, target, calls):
        calls.append(command)
        output = ""
        if command[0] == "cargo":
            output = json.dumps({"target_directory": str(target)})
        elif command[0] == "dwarfdump":
            image = Path(command[-1])
            digit = "B" if image.parent.name == "binaries" else ("A" if image.parent.parent == target else "C")
            output = f"UUID: {digit * 8}-{digit * 4}-{digit * 4}-{digit * 4}-{digit * 12} (arm64)"
        elif command[0] == "dsymutil":
            Path(command[-1]).mkdir()
        elif command[0] == "strip":
            Path(command[-1]).write_bytes(b"code")
        return subprocess.CompletedProcess(command, 0, stdout=output)

    def test_missing_release_image_fails_before_stripping(self):
        with tempfile.TemporaryDirectory(prefix=".strip-test-", dir=ROOT) as directory:
            environment = {"TAURI_ENV_PLATFORM": "darwin", "TAURI_ENV_TARGET_TRIPLE": TRIPLE}
            result = subprocess.CompletedProcess([], 0, stdout=json.dumps({"target_directory": directory}))
            with patch.dict(os.environ, environment, clear=True), patch.object(subprocess, "run", return_value=result) as run:
                with self.assertRaises(FileNotFoundError):
                    BUNDLE.main()
                self.assertEqual(run.call_count, 1)


if __name__ == "__main__":
    unittest.main()

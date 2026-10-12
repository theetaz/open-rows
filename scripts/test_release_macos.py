# SPDX-License-Identifier: Apache-2.0
"""Release assembly tests require no certificates or network service."""
from importlib import import_module
import json
from pathlib import Path
import tempfile
import unittest

config = import_module("release-config")
assets = import_module("release-artifacts")
size = import_module("check-installer-size")
ROOT = Path(__file__).resolve().parent.parent


class ReleaseTests(unittest.TestCase):
    def test_public_config_requires_key_and_repository(self):
        for env in ({}, {"TAURI_SIGNING_PUBLIC_KEY": "public"},
                    {"TAURI_SIGNING_PUBLIC_KEY": "public", "GITHUB_REPOSITORY": "../bad"}):
            with self.assertRaises(ValueError):
                config.release_config(env)
        result = config.release_config({"TAURI_SIGNING_PUBLIC_KEY": "public",
                                        "GITHUB_REPOSITORY": "example/openrows"})
        self.assertTrue(result["bundle"]["createUpdaterArtifacts"])
        self.assertEqual(result["plugins"]["updater"]["pubkey"], "public")
        self.assertNotIn("PRIVATE", json.dumps(result))

    def test_installer_budget_boundaries(self):
        with tempfile.TemporaryDirectory(dir=ROOT) as directory:
            installer = Path(directory) / "app.dmg"
            for length in (0, size.LIMIT, size.LIMIT + 1):
                with installer.open("wb") as output:
                    output.truncate(length)
                if length == size.LIMIT:
                    self.assertEqual(size.check_size(installer), size.LIMIT)
                else:
                    with self.assertRaises(ValueError):
                        size.check_size(installer)

    def test_two_architecture_release_assembly(self):
        with tempfile.TemporaryDirectory(dir=ROOT) as directory:
            root = Path(directory)
            output = root / "assets"
            for arch in ("aarch64", "x86_64"):
                bundle = root / arch
                (bundle / "dmg").mkdir(parents=True)
                (bundle / "macos").mkdir()
                (bundle / "dmg/OpenRows.dmg").write_bytes(b"installer")
                (bundle / "macos/OpenRows.app.tar.gz").write_bytes(b"archive")
                (bundle / "macos/OpenRows.app.tar.gz.sig").write_text("signature")
                assets.prepare(bundle, output, arch)
            result = assets.manifest(output, "0.1.0-beta.1", "example/openrows", "Notes")
            self.assertEqual(set(result["platforms"]), {"darwin-aarch64", "darwin-x86_64"})
            self.assertEqual(result["notes"], "Notes")
            self.assertEqual(assets.manifest(output, "0.1.0+build-linux", "example/openrows", "Notes")["version"], "0.1.0+build-linux")
            self.assertIn("/v0.1.0-beta.1/", result["platforms"]["darwin-aarch64"]["url"])
            (output / "OpenRows_x86_64.app.tar.gz.sig").unlink()
            with self.assertRaises(FileNotFoundError):
                assets.manifest(output, "0.1.0", "example/openrows", "Notes")

    def test_workflow_has_only_release_triggers(self):
        workflow = (ROOT / ".github/workflows/release.yml").read_text()
        self.assertIn("tags: ['v*']", workflow)
        self.assertIn("workflow_dispatch:", workflow)
        self.assertNotIn("pull_request:", workflow)
        self.assertNotIn("branches:", workflow)
        self.assertNotIn("mode=local", workflow)
        self.assertIn("${APPLE_CERTIFICATE:?", workflow)
        self.assertIn("${TAURI_SIGNING_PRIVATE_KEY:?", workflow)
        bundle = json.loads((ROOT / "apps/desktop/src-tauri/tauri.conf.json").read_text())["bundle"]
        self.assertTrue(bundle["macOS"]["hardenedRuntime"])
        self.assertEqual(bundle["externalBin"], ["binaries/openrows-driver-host"])


if __name__ == "__main__":
    unittest.main()

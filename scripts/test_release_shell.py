# SPDX-License-Identifier: Apache-2.0
"""Exercise local release orchestration and notarization failures with stub tools."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent


class ReleaseShellTests(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory(prefix=".release-test-", dir=ROOT)
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        (self.root / "scripts").mkdir()
        self.bin = self.root / "bin"
        self.bin.mkdir()
        for script in ("release-macos.sh", "sccache-guard.sh", "check-installer-size.py", "notarize-dmg.sh", "release-credentials.sh"):
            shutil.copyfile(ROOT / "scripts" / script, self.root / "scripts" / script)
        self.env = {**os.environ, "PATH": f"{self.bin}{os.pathsep}{os.environ['PATH']}",
                    "CARGO_TARGET_DIR": str(self.root / "target"), "RUSTC_WRAPPER": "",
                    "STUB_LOG": str(self.root / "calls.json")}

    def stub(self, name, body):
        path = self.bin / name
        path.write_text(f"#!{sys.executable}\n" + body)
        path.chmod(0o755)

    def test_local_dmg_without_secrets(self):
        self.stub("pnpm", '''import json, os, sys
from pathlib import Path
root = Path(os.environ["CARGO_TARGET_DIR"]) / os.environ["CARGO_BUILD_TARGET"] / "release/bundle"
(root / "dmg").mkdir(parents=True)
(root / "dmg/OpenRows.dmg").write_bytes(b"installer")
Path(os.environ["STUB_LOG"]).write_text(json.dumps({"args": sys.argv[1:], "apple": os.environ.get("APPLE_ID"), "key": os.environ.get("TAURI_SIGNING_PRIVATE_KEY"), "minimum": os.environ["MACOSX_DEPLOYMENT_TARGET"]}))
''')
        self.stub("codesign", "import sys\nassert '--verify' in sys.argv\n")
        self.env.update({"APPLE_ID": "fixture", "TAURI_SIGNING_PRIVATE_KEY": "fixture"})
        subprocess.run(["bash", str(self.root / "scripts/release-macos.sh"), "local", "aarch64-apple-darwin"],
                       env=self.env, check=True, capture_output=True, text=True)
        calls = json.loads((self.root / "calls.json").read_text())
        self.assertIsNone(calls["apple"])
        self.assertIsNone(calls["key"])
        self.assertEqual(calls["minimum"], "12.0")
        self.assertIn("app,dmg", calls["args"])

    def test_certificate_and_api_key_files_are_removed_on_exit(self):
        self.stub("security", '''import sys
from pathlib import Path
if sys.argv[1] == "create-keychain":
    Path(sys.argv[-1]).touch()
if sys.argv[1] == "delete-keychain":
    Path(sys.argv[-1]).unlink()
''')
        self.env.update({"RUNNER_TEMP": str(self.root), "APPLE_CERTIFICATE": "Zml4dHVyZQ==",
                         "APPLE_CERTIFICATE_PASSWORD": "fixture", "APPLE_SIGNING_IDENTITY": "fixture",
                         "APPLE_API_PRIVATE_KEY": "fixture"})
        subprocess.run(["bash", "-c", 'source "$1"; test -f "$APPLE_API_KEY_PATH"', "release-test",
                        str(self.root / "scripts/release-credentials.sh")],
                       env=self.env, check=True, capture_output=True, text=True)
        self.assertEqual(list(self.root.glob("openrows-signing.*")), [])

    def test_rejected_notarization_fails_before_stapling(self):
        self.stub("codesign", "")
        self.stub("xcrun", '''import json, sys
assert sys.argv[1] == "notarytool", "Rejected submissions must not staple"
print(json.dumps({"status": "Invalid"}))
''')
        self.env.update({"APPLE_SIGNING_IDENTITY": "fixture", "APPLE_ID": "fixture",
                         "APPLE_TEAM_ID": "fixture", "APPLE_PASSWORD": "fixture"})
        result = subprocess.run(["bash", str(self.root / "scripts/notarize-dmg.sh"), "fixture.dmg"],
                                env=self.env, capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Invalid", result.stderr)

    def preflight(self, **variables):
        env = {key: value for key, value in self.env.items()
               if not key.startswith(("APPLE_", "TAURI_SIGNING_", "GITHUB_OUTPUT"))}
        return subprocess.run(["bash", str(ROOT / "scripts/release-preflight.sh")],
                              env={**env, **variables}, capture_output=True, text=True)

    def test_preflight_skips_absent_credentials_and_rejects_partial_ones(self):
        absent = self.preflight()
        self.assertEqual(absent.returncode, 0, absent.stderr)
        self.assertIn("apple=false\nupdater=false\npublish=false", absent.stdout)
        self.assertIn("Signing skipped", absent.stdout)
        certificate = {"APPLE_CERTIFICATE": "c2VjcmV0", "APPLE_CERTIFICATE_PASSWORD": "pw-fixture",
                       "APPLE_SIGNING_IDENTITY": "Developer ID Application: Fixture (TEAM)"}
        account = {"APPLE_ID": "id-fixture", "APPLE_TEAM_ID": "TEAM", "APPLE_PASSWORD": "app-fixture"}
        updater = {"TAURI_SIGNING_PRIVATE_KEY": "key-fixture", "TAURI_SIGNING_PUBLIC_KEY": "public"}
        complete = self.preflight(**certificate, **account, **updater)
        self.assertIn("apple=true\nupdater=true\npublish=true", complete.stdout)
        for partial, named in (({**certificate}, "APPLE_ID"),
                               ({**certificate, **account, "APPLE_SIGNING_IDENTITY": "-"}, "ad-hoc"),
                               ({"APPLE_API_KEY": "key-id"}, "APPLE_CERTIFICATE"),
                               ({"TAURI_SIGNING_PRIVATE_KEY": "key-fixture"}, "TAURI_SIGNING_PUBLIC_KEY")):
            result = self.preflight(**partial)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn(named, result.stderr)
            for value in ("c2VjcmV0", "pw-fixture", "app-fixture", "key-fixture"):
                self.assertNotIn(value, result.stdout + result.stderr)

    def test_publication_resumes_drafts_and_refuses_published_assets(self):
        workflow = (ROOT / ".github/workflows/release.yml").read_text()
        block = workflow.split('      - name: Create draft release and upload assets\n        run: |\n', 1)[1]
        block = block.split('          gh release view "$RELEASE_TAG" --json body', 1)[0]
        commands = "\n".join(line[10:] for line in block.splitlines())
        self.stub("gh", '''import os, sys
from pathlib import Path
if sys.argv[2] == "view":
    if os.environ["RELEASE_STATE"] == "missing":
        sys.exit(1)
    if "--json" in sys.argv:
        print(os.environ["RELEASE_STATE"])
else:
    Path(os.environ["STUB_LOG"]).write_text("created")
''')
        for state in ("true", "false", "missing"):
            env = {**self.env, "RELEASE_TAG": "v0.1.0", "RELEASE_STATE": state}
            result = subprocess.run(["bash", "-euo", "pipefail", "-c", commands],
                                    env=env, capture_output=True, text=True)
            self.assertEqual(result.returncode == 0, state != "false", result.stderr)
            self.assertEqual((self.root / "calls.json").exists(), state == "missing")
            if state == "false":
                self.assertIn("Refusing to overwrite", result.stderr)


if __name__ == "__main__":
    unittest.main()

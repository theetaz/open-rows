# SPDX-License-Identifier: Apache-2.0
"""Name per-architecture assets and generate Tauri's signed static manifest."""
from datetime import datetime, timezone
import json
from pathlib import Path
import re
import shutil
import sys

from importlib import import_module
check_size = import_module("check-installer-size").check_size


def prepare(bundle, destination, arch):
    """Copy only installer and signed updater assets, never build symbols."""
    if arch not in ("aarch64", "x86_64"):
        raise ValueError("Unsupported architecture")
    destination.mkdir(parents=True, exist_ok=True)
    installers = list((bundle / "dmg").glob("*.dmg"))
    if len(installers) != 1:
        raise ValueError("Expected exactly one DMG")
    check_size(installers[0])
    shutil.copy2(installers[0], destination / f"OpenRows_{arch}.dmg")
    archive = bundle / "macos/OpenRows.app.tar.gz"
    if archive.exists():
        signature = Path(str(archive) + ".sig")
        if not signature.read_text().strip():
            raise ValueError("Missing updater signature")
        name = f"OpenRows_{arch}.app.tar.gz"
        shutil.copy2(archive, destination / name)
        shutil.copy2(signature, destination / f"{name}.sig")


def manifest(folder, version, repository, notes):
    """Refuse partial releases; every platform must have a signed archive."""
    if not re.fullmatch(r"\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?", version):
        raise ValueError("Invalid release version")
    if not re.fullmatch(r"[A-Za-z0-9][\w.-]*/[A-Za-z0-9][\w.-]*", repository):
        raise ValueError("Invalid GitHub repository")
    platforms = {}
    for arch in ("aarch64", "x86_64"):
        name = f"OpenRows_{arch}.app.tar.gz"
        archive = folder / name
        signature = (folder / f"{name}.sig").read_text().strip()
        if not archive.is_file() or archive.stat().st_size == 0 or not signature:
            raise ValueError("Missing signed updater artifact")
        platforms[f"darwin-{arch}"] = {
            "signature": signature,
            "url": f"https://github.com/{repository}/releases/download/v{version}/{name}",
        }
    return {"version": version, "notes": notes,
            "pub_date": datetime.now(timezone.utc).isoformat(), "platforms": platforms}


if __name__ == "__main__":
    if sys.argv[1] == "prepare":
        prepare(Path(sys.argv[2]), Path(sys.argv[3]), sys.argv[4])
    elif sys.argv[1] == "manifest":
        folder = Path(sys.argv[2])
        result = manifest(folder, sys.argv[3], sys.argv[4], Path(sys.argv[5]).read_text())
        channel = "beta" if "-" in result["version"].split("+", 1)[0] else "stable"
        (folder / f"{channel}.json").write_text(json.dumps(result, indent=2) + "\n")
    else:
        raise SystemExit("Usage: release-artifacts.py prepare|manifest ...")

# SPDX-License-Identifier: Apache-2.0
"""Keep release symbols outside the signed bundle to reduce its first-exec scan."""
import json
import os
from pathlib import Path
import re
import shutil
import subprocess

ROOT = Path(__file__).resolve().parent.parent


def strip_binary(binary, target):
    """Archive the image's symbols once per UUID before removing its symbol table."""
    result = subprocess.run(["dwarfdump", "--uuid", str(binary)], check=True,
                            capture_output=True, text=True)
    match = re.search(r"UUID: ([0-9A-Fa-f-]{36})", result.stdout)
    if match is None:
        raise ValueError(f"Missing Mach-O UUID: {binary.name}")
    symbols = target / "bundle-symbols" / f"{match.group(1)}.dSYM"
    symbols.parent.mkdir(parents=True, exist_ok=True)
    image = symbols.with_suffix(".macho")
    if not image.exists():
        partial = image.with_suffix(".partial")
        try:
            shutil.copy2(binary, partial)
            partial.replace(image)
        finally:
            partial.unlink(missing_ok=True)
    if not symbols.exists():
        subprocess.run(["dsymutil", str(binary), "-o", str(symbols)], check=True)
    subprocess.run(["strip", str(binary)], check=True)
    # Stripping invalidates the linker signature; cached images must stay runnable.
    subprocess.run(["codesign", "--force", "--sign", "-", str(binary)], check=True)


def main():
    """Tauri calls this after compilation and before copying/signing release code."""
    if os.environ.get("TAURI_ENV_PLATFORM") != "darwin" or os.environ.get("TAURI_ENV_DEBUG") == "true":
        return
    triple = os.environ["TAURI_ENV_TARGET_TRIPLE"]
    if not re.fullmatch(r"[a-zA-Z0-9_-]+", triple):
        raise ValueError("Invalid Tauri target triple")
    metadata = subprocess.run(["cargo", "metadata", "--no-deps", "--locked", "--format-version", "1"],
                              cwd=ROOT, check=True, capture_output=True, text=True)
    target = Path(json.loads(metadata.stdout)["target_directory"])
    # Cargo places native/no-target and explicit-target builds in different folders.
    # Strip both existing release images; debug/test binaries are never selected.
    apps = [folder / "release/openrows-app" for folder in (target, target / triple)]
    apps = [app for app in apps if app.is_file()]
    if not apps:
        raise FileNotFoundError("Release application missing before bundling")
    helper = ROOT / "apps/desktop/src-tauri/binaries" / f"openrows-driver-host-{triple}"
    for binary in [*apps, helper]:
        strip_binary(binary, target)
    print(f"strip-macos-bundle: stripped {len(apps)} release app image(s) and helper; dSYMs retained")


if __name__ == "__main__":
    main()

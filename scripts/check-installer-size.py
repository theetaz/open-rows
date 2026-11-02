# SPDX-License-Identifier: Apache-2.0
"""Enforce the NFR-PORT-02 decimal 40 MB installer ceiling."""
from pathlib import Path
import sys

LIMIT = 40_000_000


def check_size(path):
    """Reject empty/missing installers and anything above the release budget."""
    size = Path(path).stat().st_size
    if not 0 < size <= LIMIT:
        raise ValueError(f"{Path(path).name}: {size} bytes exceeds installer budget (1..{LIMIT})")
    return size


if __name__ == "__main__":
    for argument in sys.argv[1:]:
        print(f"installer-size: {Path(argument).name}: {check_size(argument)} bytes <= {LIMIT}")
    if len(sys.argv) < 2:
        raise SystemExit("Usage: check-installer-size.py installer.dmg [...]")

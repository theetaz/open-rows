# SPDX-License-Identifier: Apache-2.0
"""macOS process attribution, including launchd-parented WebKit XPC services."""
import ctypes
from datetime import datetime
import os
import re
import shutil
import subprocess

from bench_metrics import BenchmarkError

WEBKIT = ("com.apple.WebKit.WebContent", "com.apple.WebKit.GPU", "com.apple.WebKit.Networking")


def process_table():
    """RSS from ps is KiB; start time guards cleanup against PID reuse."""
    result = subprocess.run(["ps", "-U", str(os.getuid()), "-xo",
                             "pid=,ppid=,pgid=,rss=,stat=,lstart=,comm="],
                            check=True, capture_output=True, text=True, timeout=10,
                            env=dict(os.environ, LC_ALL="C"))
    rows = {}
    for line in result.stdout.splitlines():
        parts = line.split(maxsplit=10)
        if len(parts) != 11:
            raise BenchmarkError(f"Unrecognized ps process row: {line}")
        if parts[4].startswith("Z"):
            continue
        pid, parent, group, rss = map(int, parts[:4])
        identity = " ".join(parts[5:10])
        rows[pid] = dict(pid=pid, parent=parent, group=group, rss_bytes=rss * 1024,
                         identity=identity, name=parts[10],
                         started_at=datetime.strptime(identity, "%a %b %d %H:%M:%S %Y").timestamp())
    return rows


class Responsibility:
    """Resolve macOS XPC ownership, rather than charging every WebKit process.

    This libSystem symbol is exported by the macOS SDK but is not a public API.
    If it is unavailable, fail explicitly rather than undercounting memory.
    """
    def __init__(self):
        try:
            library = ctypes.CDLL("/usr/lib/libSystem.B.dylib")
            self.owner = library.responsibility_get_pid_responsible_for_pid
            self.owner.argtypes = [ctypes.c_int]
            self.owner.restype = ctypes.c_int
        except (OSError, AttributeError) as error:
            raise BenchmarkError("macOS XPC responsibility API unavailable") from error

    def __call__(self, pid):
        owner = self.owner(pid)
        if owner <= 0:
            raise BenchmarkError(f"Cannot determine responsibility for live WebKit PID {pid}")
        return owner


def webkit_owner_since(row, start, owner):
    """Only query responsibility for WebKit processes started with this launch."""
    webkit = any(name in row["name"] for name in WEBKIT)
    if webkit and row["started_at"] >= start:
        return owner(row["pid"])
    return None


def app_processes(rows, root, owner):
    """Select app processes and new XPCs with direct or inherited responsibility."""
    if root not in rows:
        return []
    root_owner = owner(root)
    root_start = rows[root]["started_at"]
    selected = {root}
    changed = True
    while changed:
        before = len(selected)
        for pid, row in rows.items():
            descendant = row["parent"] in selected or row["group"] == root
            responsible_pid = webkit_owner_since(row, root_start, owner)
            if descendant or responsible_pid == root_owner or responsible_pid in selected:
                selected.add(pid)
        changed = len(selected) != before
    return [rows[pid] for pid in sorted(selected) if pid in rows]


def require_webcontent(processes):
    """Reject a deceptively small total if XPC attribution missed WebContent."""
    if not any(WEBKIT[0] in row["name"] for row in processes):
        raise BenchmarkError("No app-owned WebContent found; cannot certify all-process memory")


def footprint_bytes(output):
    """Parse footprint's byte-format process headline, not peak or region totals."""
    match = re.search(r"Footprint:\s*([\d,]+)\s*(?:bytes|B)\b", output)
    if not match:
        raise BenchmarkError("Cannot parse footprint byte-format headline")
    return int(match[1].replace(",", ""))


def add_footprints(processes):
    """Collect required footprint samples; retain failures for the budget verdict."""
    executable = shutil.which("footprint")
    for row in processes:
        row["footprint_bytes"] = None
        if executable is None:
            row["footprint_unavailable"] = "footprint is not installed"
            continue
        try:
            result = subprocess.run([executable, "-f", "bytes", "-p", str(row["pid"])],
                                    check=True, capture_output=True, text=True, timeout=15)
            row["footprint_bytes"] = footprint_bytes(result.stdout)
        except (subprocess.SubprocessError, OSError, BenchmarkError) as error:
            row["footprint_unavailable"] = str(error)


def signal_if_same(row, current, signum):
    """Never signal a reused PID or an unrelated browser process."""
    other = current.get(row["pid"])
    if other and (other["identity"], other["name"]) == (row["identity"], row["name"]):
        try:
            os.kill(row["pid"], signum)
        except ProcessLookupError:
            pass  # Exit between the snapshot and signal is successful cleanup.

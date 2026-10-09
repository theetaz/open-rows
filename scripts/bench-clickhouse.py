#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Measure native HTTP Arrow ingest, excluding compilation from peak RSS."""
import json
import os
import re
from pathlib import Path
import subprocess
import sys


def peak_rss(output, platform):
    """Read bytes from Darwin time or KiB from GNU time; fail on absent evidence."""
    pattern = r"(\d+)\s+maximum resident set size" if platform == "darwin" else r"Maximum resident set size \(kbytes\):\s*(\d+)"
    match = re.search(pattern, output)
    if not match or int(match.group(1)) <= 0:
        raise RuntimeError("Missing positive peak RSS measurement")
    return int(match.group(1)) * (1 if platform == "darwin" else 1024)


def main():
    root = Path(__file__).resolve().parent.parent
    build = subprocess.run(
        ["cargo", "test", "-p", "openrows-driver-clickhouse", "--test", "clickhouse",
         "--no-run", "--locked", "--message-format=json"],
        cwd=root, capture_output=True, text=True, check=True,
    )
    artifacts = [json.loads(line) for line in build.stdout.splitlines() if line.startswith("{")]
    binaries = [item["executable"] for item in artifacts
                if item.get("reason") == "compiler-artifact" and item.get("executable")
                and item["target"]["name"] == "clickhouse"]
    if len(binaries) != 1:
        raise RuntimeError("Expected one ClickHouse benchmark test binary")
    command = [binaries[0], "streaming::million_row_arrow_ingest_benchmark", "--exact", "--ignored", "--nocapture"]
    if sys.platform == "darwin":
        command = ["/usr/bin/time", "-l", *command]
    elif Path("/usr/bin/time").exists():
        command = ["/usr/bin/time", "-v", *command]
    else:
        raise RuntimeError("/usr/bin/time is required to measure peak memory")
    result = subprocess.run(command, cwd=root, env=os.environ.copy(), capture_output=True, text=True)
    print(result.stdout, end="")
    print(result.stderr, end="", file=sys.stderr)
    result.check_returncode()
    peak = peak_rss(result.stderr, sys.platform)
    print(f"ClickHouse ingest peak_rss_bytes={peak} (whole test process; driver buffer target 4 MiB)")
    if peak > 40_000_000:
        raise RuntimeError("Ingest process peak RSS exceeds the 40 MB core baseline budget")


if __name__ == "__main__":
    main()

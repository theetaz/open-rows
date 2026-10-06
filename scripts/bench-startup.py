# SPDX-License-Identifier: Apache-2.0
"""Fresh-process startup samples; OS disk caches are intentionally not purged."""
import json
import os
from pathlib import Path
import statistics
import subprocess
import sys
import tempfile
import time


def launch(executable, report):
    """Include process creation and linking; stop at the Rust paint-ready receipt."""
    environment = dict(os.environ, OPENROWS_STARTUP_REPORT=str(report))
    started = time.time_ns()
    process = subprocess.Popen([executable], env=environment)
    try:
        deadline = time.monotonic() + 30
        while not report.exists() or report.stat().st_size == 0:
            if process.poll() is not None:
                raise RuntimeError(f"App exited before first paint: {process.returncode}")
            if time.monotonic() > deadline:
                raise TimeoutError("No first-paint-ready event within 30 seconds")
        return (int(report.read_text()) - started) / 1_000_000
    finally:
        if process.poll() is None:
            process.terminate()
        process.wait(timeout=10)


def main():
    """Launch five sequential processes and remove their reports on exit."""
    executable = str(Path(sys.argv[1]).resolve(strict=True))
    with tempfile.TemporaryDirectory(prefix=".startup-", dir=Path.cwd()) as directory:
        samples = [launch(executable, Path(directory) / str(index)) for index in range(5)]
    print(json.dumps({"samples_ms": samples, "median_ms": statistics.median(samples)}, indent=2))


if __name__ == "__main__":
    main()

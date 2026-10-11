# SPDX-License-Identifier: Apache-2.0
"""Repeat hosted conformance binaries under bounded, native CPU contention."""

import argparse
from pathlib import Path
import subprocess
import sys
import tempfile

LOAD = """
import hashlib
data = bytes(65536)
print("ready", flush=True)
while True:
    hashlib.sha256(data).digest()
"""


def arguments():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--runs", type=int, default=50)
    parser.add_argument("--cpu-workers", type=int, choices=range(1, 5), default=4)
    args = parser.parse_args()
    if args.runs < 1:
        parser.error("--runs must be positive")
    return args


def stop(workers):
    for worker in workers:
        worker.terminate()
    for worker in workers:
        try:
            worker.wait(timeout=5)
        except subprocess.TimeoutExpired:
            worker.kill()
            worker.wait()
        if worker.stdout:
            worker.stdout.close()


def repeat(metadata, args, root):
    workers = []
    try:
        for _ in range(args.cpu_workers):
            worker = subprocess.Popen(
                [sys.executable, "-c", LOAD], stdin=subprocess.DEVNULL,
                stdout=subprocess.PIPE, text=True,
            )
            workers.append(worker)
            if worker.stdout.readline().strip() != "ready":
                raise RuntimeError("CPU load worker did not start")
        print(f"Controlled load: {len(workers)} SHA-256 CPU workers", flush=True)
        return subprocess.call([
            "cargo", "nextest", "run", "--binaries-metadata", str(metadata),
            "-E", "binary(=conformance) | binary(=duckdb_conformance)",
            "--stress-count", str(args.runs), "--no-fail-fast",
        ], cwd=root)
    finally:
        stop(workers)


def main():
    args = arguments()
    root = Path(__file__).resolve().parents[1]
    # Compile before starting CPU load; reuse the caller's shared Cargo cache.
    with tempfile.TemporaryDirectory(prefix=".host-test-load-", dir=root) as directory:
        metadata = Path(directory) / "binaries.json"
        with metadata.open("w") as output:
            built = subprocess.call([
                "cargo", "nextest", "list", "-p", "openrows-driver-host",
                "--locked", "--list-type", "binaries-only", "--message-format", "json",
            ], cwd=root, stdout=output)
        if built:
            return built
        return repeat(metadata, args, root)


if __name__ == "__main__":
    sys.exit(main())

# SPDX-License-Identifier: Apache-2.0
"""Five fresh app copies: visible paint/idle → Probe → real DuckDB connect/query/close."""
import json
import os
from pathlib import Path
import subprocess
import tempfile

from bench_common import ROOT, launch, parser, read_report, release_executable, run, write_result
from bench_metrics import BenchmarkError, print_budgets, summary


def measure(source, root, index, fresh_signature):
    app = root / f"OpenRows-{index}.app"
    subprocess.run(["ditto", str(source), str(app)], check=True)
    helper = app / "Contents/MacOS/openrows-driver-host"
    if fresh_signature:
        identity = f"com.prabhavalabs.openrows.host-benchmark-{root.name}-{index}"
        subprocess.run(["codesign", "--force", "--sign", "-", "--options", "runtime",
                        "--identifier", identity, str(helper)], check=True, capture_output=True)
        subprocess.run(["codesign", "--force", "--sign", "-", "--options", "runtime", str(app)],
                       check=True, capture_output=True)
        subprocess.run(["codesign", "--verify", "--deep", "--strict", str(app)], check=True)
    quarantine = "0081;67000000;test-only;"
    applied = subprocess.run(["xattr", "-w", "com.apple.quarantine", quarantine, str(helper)],
                             capture_output=True).returncode == 0
    report = root / f"host-{index}.json"
    os.environ["OPENROWS_HOST_BENCH_REPORT"] = str(report)
    try:
        with launch(app / "Contents/MacOS/openrows-app") as (process, startup):
            host = read_report(process, report)
            if host.get("error") or host.get("pid") != process.pid:
                raise BenchmarkError(f"Invalid native connect report: {host}")
    finally:
        os.environ.pop("OPENROWS_HOST_BENCH_REPORT", None)
    retained = subprocess.run(["xattr", "-p", "com.apple.quarantine", str(helper)],
                              capture_output=True, text=True)
    return dict(host, startup=startup, fresh_signature=fresh_signature, quarantine_applied=applied,
                quarantine_retained=retained.returncode == 0 and retained.stdout.strip() == quarantine)


def main():
    arguments = parser(__doc__)
    arguments.add_argument("runs", type=int, nargs="?", default=5)
    arguments.add_argument("--fresh-signature", action="store_true",
                           help="Ad-hoc re-sign each temporary helper with a distinct identity, then reseal its app")
    options = arguments.parse_args()
    if options.runs < 2:
        arguments.error("at least two fresh copies are required")
    executable = release_executable(options.executable)
    source = executable.parents[2]
    with tempfile.TemporaryDirectory(prefix=".bench-duckdb-", dir=ROOT) as directory:
        samples = []
        for index in range(1, options.runs + 1):
            sample = measure(source, Path(directory), index, options.fresh_signature)
            samples.append(sample)
            print(f'{index}: paint={sample["startup"]["total_ms"]:.3f} ms; '
                  f'prewarm={sample["prewarm_ms"]:.3f} ms ({sample["prewarm_outcome"]}); '
                  f'first handshake={sample["first_connect_handshake_ms"]:.3f} ms; '
                  f'connect={sample["first_connect_ms"]:.3f} ms; '
                  f'quarantine retained={sample["quarantine_retained"]}', flush=True)
    # Every copy is fresh: treating copies 2..N as warm app launches is misleading.
    cold = summary([sample["startup"]["total_ms"] for sample in samples])["p95"]
    handshake = summary([sample["first_connect_handshake_ms"] for sample in samples])["p95"]
    budgets = [dict(metric="fresh-copy window p95", actual=cold, limit=800, unit="ms", passed=cold <= 800),
               dict(metric="prewarmed first handshake p95", actual=handshake, limit=75,
                    unit="ms", passed=handshake <= 75)]
    write_result("duckdb-prewarm", dict(samples=samples, budgets=budgets,
                                       method="Fresh ditto copies with helper quarantine where available; "
                                              "real app visible paint/idle, Probe handshake/close/reap, "
                                              "production runtime in-memory DuckDB connect, SELECT 1, close; "
                                              "parent monotonic clocks; no OS cache purge or retries"))
    print(json.dumps(budgets))
    return print_budgets(budgets)


if __name__ == "__main__":
    raise SystemExit(run(main))

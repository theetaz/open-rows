# SPDX-License-Identifier: Apache-2.0
"""Gate the last Criterion decode_pg_* runs from the shared Cargo target; build nothing."""
import json
import subprocess
from pathlib import Path
from bench_common import ROOT, run
from bench_metrics import BenchmarkError, print_budgets
from bench_pg_metrics import (FIRST_BATCH_BENCH, THROUGHPUT_BENCHES, first_batch_budget,
                              throughput_budget)


def latest(criterion, name):
    """Each group has exactly one benchmark function; return its newest estimates."""
    runs = sorted((criterion / name).glob("*/new"))
    if len(runs) != 1:
        raise BenchmarkError(f"Expected one Criterion result for {name}, found {len(runs)}")
    return tuple(json.loads((runs[0] / file).read_text())
                 for file in ("estimates.json", "benchmark.json"))


def main():
    metadata = subprocess.run(["cargo", "metadata", "--no-deps", "--locked", "--format-version", "1"],
                              cwd=ROOT, check=True, capture_output=True, text=True, timeout=30)
    criterion = Path(json.loads(metadata.stdout)["target_directory"]) / "criterion"
    budgets = [throughput_budget(name, *latest(criterion, name)) for name in THROUGHPUT_BENCHES]
    budgets.append(first_batch_budget(latest(criterion, FIRST_BATCH_BENCH)[0]))
    return print_budgets(budgets)


if __name__ == "__main__":
    raise SystemExit(run(main))

# SPDX-License-Identifier: Apache-2.0
"""Gate the last Criterion 256x20 run from the shared Cargo target; build nothing."""
import json
import subprocess
from pathlib import Path
from bench_common import ROOT, run
from bench_metrics import print_budgets
from bench_ipc_metrics import encode_budget


def main():
    metadata = subprocess.run(["cargo", "metadata", "--no-deps", "--locked", "--format-version", "1"],
                              cwd=ROOT, check=True, capture_output=True, text=True, timeout=30)
    target = Path(json.loads(metadata.stdout)["target_directory"])
    path = target / "criterion/result_window_encode/256x20/new/estimates.json"
    budget = encode_budget(json.loads(path.read_text()))
    return print_budgets([budget])


if __name__ == "__main__":
    raise SystemExit(run(main))

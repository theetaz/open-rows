# SPDX-License-Identifier: Apache-2.0
"""Gate the last Criterion schema_* runs from the shared Cargo target; build nothing."""
import json
import subprocess
from pathlib import Path
from bench_common import ROOT, run
from bench_metrics import print_budgets
from bench_schema_metrics import schema_budgets


def main():
    metadata = subprocess.run(["cargo", "metadata", "--no-deps", "--locked", "--format-version", "1"],
                              cwd=ROOT, check=True, capture_output=True, text=True, timeout=30)
    criterion = Path(json.loads(metadata.stdout)["target_directory"]) / "criterion"
    return print_budgets(schema_budgets(
        lambda group, function: json.loads(
            (criterion / group / function / "new/estimates.json").read_text())))


if __name__ == "__main__":
    raise SystemExit(run(main))

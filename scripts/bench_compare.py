# SPDX-License-Identifier: Apache-2.0
"""Compare Criterion results against a saved `main` baseline (docs/25 §5.1 rule 4).

Run the benchmarks on the baseline with `-- --save-baseline main`, then on the
change with `-- --baseline main`; Criterion writes each benchmark's relative
change to `<target>/criterion/<id>/change/estimates.json`. A benchmark fails
only when the lower bound of the 95 % confidence interval of its mean change
exceeds its threshold, so runner noise inside the interval never fails a run.
"""
import json
from pathlib import Path
import sys

DEFAULT_THRESHOLD = 0.05
# docs/13 §4: the SQL splitter's gate is −10 % throughput.
THRESHOLDS = {"sql_split": 0.10}


class CompareError(Exception):
    """No comparable results, or a malformed Criterion estimate."""


def threshold(benchmark):
    group = benchmark.split("/", 1)[0]
    return next((value for prefix, value in THRESHOLDS.items() if group.startswith(prefix)),
                DEFAULT_THRESHOLD)


def mean_change(path):
    """Relative mean change and its 95 % confidence interval from one estimates file."""
    data = json.loads(path.read_text())
    mean = data.get("mean") if isinstance(data, dict) else None
    interval = mean.get("confidence_interval") if isinstance(mean, dict) else None
    values = [mean.get("point_estimate") if mean else None] + [
        interval.get(key) if isinstance(interval, dict) else None for key in ("lower_bound", "upper_bound")]
    if not all(isinstance(value, (int, float)) for value in values):
        raise CompareError(f"Malformed Criterion change estimate: {path}")
    return dict(zip(("change", "lower", "upper"), values))


def compare(criterion):
    """Every benchmark with a change estimate, sorted by id."""
    rows = []
    for path in sorted(Path(criterion).glob("**/change/estimates.json")):
        benchmark = path.parent.parent.relative_to(criterion).as_posix()
        change = mean_change(path)
        limit = threshold(benchmark)
        rows.append(dict(change, benchmark=benchmark, threshold=limit, regressed=change["lower"] > limit))
    if not rows:
        raise CompareError(f"No Criterion change estimates under {criterion}; run with --baseline first")
    return rows


def print_rows(rows):
    print(f'{"Benchmark":<48}{"Change":>9}{"95% CI":>20}{"Limit":>8}')
    for row in rows:
        interval = f'[{row["lower"]:+.1%}, {row["upper"]:+.1%}]'
        verdict = "REGRESSED" if row["regressed"] else "ok"
        print(f'{row["benchmark"]:<48}{row["change"]:>+9.1%}{interval:>20}{row["threshold"]:>+8.0%}  {verdict}')
    regressed = sum(row["regressed"] for row in rows)
    print(f"{regressed} of {len(rows)} benchmarks regressed beyond their threshold")
    return 1 if regressed else 0


def main(arguments):
    if len(arguments) != 1:
        print("usage: bench_compare.py <cargo target directory>/criterion")
        return 2
    try:
        return print_rows(compare(arguments[0]))
    except (CompareError, OSError, ValueError) as error:
        print(f"Benchmark comparison failed: {error}")
        return 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))

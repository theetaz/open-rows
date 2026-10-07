# SPDX-License-Identifier: Apache-2.0
"""Gate Criterion decode_pg_* estimates: cells/s per core and first-batch decode time."""
import math
from bench_metrics import BenchmarkError

THROUGHPUT_BENCHES = ("decode_pg_int", "decode_pg_text", "decode_pg_mixed", "decode_pg_wide")
FIRST_BATCH_BENCH = "decode_pg_first_batch"
MIN_MILLION_CELLS_PER_SECOND = 1
FIRST_BATCH_LIMIT_MS = 5


def mean_upper_ns(estimates):
    """Upper 95% confidence bound of the mean iteration time: the pessimistic estimate."""
    try:
        actual = estimates["mean"]["confidence_interval"]["upper_bound"]
    except (KeyError, TypeError) as error:
        raise BenchmarkError("Missing Criterion mean confidence interval") from error
    if type(actual) not in (int, float) or not math.isfinite(actual) or actual <= 0:
        raise BenchmarkError("Invalid Criterion decode estimate")
    return actual


def cells_per_iteration(benchmark):
    try:
        cells = benchmark["throughput"]["Elements"]
    except (KeyError, TypeError) as error:
        raise BenchmarkError("Missing Criterion element throughput") from error
    if type(cells) is not int or cells <= 0:
        raise BenchmarkError("Invalid Criterion element throughput")
    return cells


def throughput_budget(name, estimates, benchmark):
    """Criterion runs single-threaded, so cells/s is per core (NFR-PERF-05)."""
    actual = cells_per_iteration(benchmark) / mean_upper_ns(estimates) * 1_000
    return dict(metric=f"{name} worst-case throughput", actual=actual,
                limit=MIN_MILLION_CELLS_PER_SECOND, unit="M cells/s",
                passed=actual >= MIN_MILLION_CELLS_PER_SECOND)


def first_batch_budget(estimates):
    actual = mean_upper_ns(estimates) / 1_000_000
    return dict(metric=f"{FIRST_BATCH_BENCH} mean upper bound", actual=actual,
                limit=FIRST_BATCH_LIMIT_MS, unit="ms", passed=actual <= FIRST_BATCH_LIMIT_MS)

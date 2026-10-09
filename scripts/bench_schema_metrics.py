# SPDX-License-Identifier: Apache-2.0
"""Gate Criterion schema_* estimates at 50 000 objects (M1-16, NFR-PERF-08)."""
from bench_pg_metrics import mean_upper_ns

FIRST_LEVEL_BENCH = ("schema_first_level_50k", "snapshot_restore")
SEARCH_GROUP = "schema_search_50k"
SEARCH_BENCHES = ("exact", "prefix", "substring", "subsequence", "schema_filter")
FIRST_LEVEL_LIMIT_MS = 300
SEARCH_LIMIT_MS = 50


def mean_upper_budget(name, estimates, limit_ms):
    """The upper 95% bound of Criterion's mean must not exceed the limit."""
    actual = mean_upper_ns(estimates) / 1_000_000
    return dict(metric=f"{name} mean upper bound", actual=actual, limit=limit_ms,
                unit="ms", passed=actual <= limit_ms)


def schema_budgets(read):
    """`read(group, function)` returns that benchmark's estimates.json."""
    group, function = FIRST_LEVEL_BENCH
    budgets = [mean_upper_budget(f"{group}/{function}", read(group, function),
                                 FIRST_LEVEL_LIMIT_MS)]
    budgets += [mean_upper_budget(f"{SEARCH_GROUP}/{name}", read(SEARCH_GROUP, name),
                                  SEARCH_LIMIT_MS) for name in SEARCH_BENCHES]
    return budgets

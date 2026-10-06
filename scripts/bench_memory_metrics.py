# SPDX-License-Identifier: Apache-2.0
"""Idle-memory aggregation: sum each launch before taking medians across launches."""
import statistics

from bench_metrics import BenchmarkError, memory_budget


def process_name(process):
    return process["name"].split("/")[-1]


def memory_sample(rows):
    """Invalid footprints remain explicit; no median may hide an incomplete launch."""
    budget = memory_budget(rows)
    complete = budget["actual"] is not None
    return dict(processes=[dict(row, name=process_name(row)) for row in rows],
                total_rss_bytes=sum(row["rss_bytes"] for row in rows),
                total_footprint_bytes=(sum(row["footprint_bytes"] for row in rows)
                                       if complete else None),
                webview_footprint_bytes=(sum(row["footprint_bytes"] for row in rows
                                            if process_name(row).startswith("com.apple.WebKit."))
                                         if complete else None),
                error=budget.get("error"))


def memory_report(samples):
    """Per-process medians are diagnostic; gates use medians of per-run totals."""
    if not samples:
        raise BenchmarkError("At least one memory run is required")
    errors = [f'Run {index + 1}: {sample["error"]}'
              for index, sample in enumerate(samples) if sample["error"]]
    budgets = []
    for key, metric, limit in (
        ("total_footprint_bytes", "median total physical footprint", 200),
        ("webview_footprint_bytes", "median webview physical footprint", 140),
    ):
        actual = None if errors else statistics.median([sample[key] for sample in samples]) / 1_000_000
        budget = dict(metric=metric, actual=actual, limit=limit, unit="MB",
                      passed=actual is not None and actual <= limit)
        if errors:
            budget["error"] = "; ".join(errors)
        budgets.append(budget)
    names = sorted({row["name"] for sample in samples for row in sample["processes"]})
    medians = {name: statistics.median([
        sum(row["footprint_bytes"] for row in sample["processes"] if row["name"] == name)
        for sample in samples]) for name in names} if not errors else None
    return dict(samples=samples, budgets=budgets, process_median_footprint_bytes=medians,
                median_total_footprint_bytes=None if errors else budgets[0]["actual"] * 1_000_000,
                median_webview_footprint_bytes=None if errors else budgets[1]["actual"] * 1_000_000,
                gated_metrics=["median_total_footprint_bytes", "median_webview_footprint_bytes"])

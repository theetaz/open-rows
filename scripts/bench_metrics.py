# SPDX-License-Identifier: Apache-2.0
"""Pure report validation, statistics and NFR-PERF budget evaluation."""
import math
import statistics

PHASES = (
    "main_entry", "builder_start", "webview_created", "setup_complete",
    "page_load_finished", "first_paint_ready", "window_shown",
)
MEMORY_BUDGET_BYTES = 200_000_000


class BenchmarkError(RuntimeError):
    """An invalid measurement must not be treated as a passing benchmark."""


def percentile(samples, fraction):
    """Nearest-rank percentile: a small sample never hides its slowest run."""
    if not samples or not all(math.isfinite(x) and x >= 0 for x in samples):
        raise BenchmarkError("Expected nonempty, finite, nonnegative samples")
    return sorted(samples)[max(0, math.ceil(len(samples) * fraction) - 1)]


def summary(samples):
    """Keep p50 as the conventional median, p95 as nearest rank."""
    percentile(samples, 0.95)
    return dict(min=min(samples), p50=statistics.median(samples),
                p95=percentile(samples, 0.95), max=max(samples))


def startup_sample(report, started_ns, pid):
    """Use the same OS CLOCK_MONOTONIC epoch in Rust and Python."""
    if not isinstance(report, dict):
        raise BenchmarkError("Expected a startup report object")
    if (report.get("schema_version") != 1 or report.get("clock") != "CLOCK_MONOTONIC"
            or report.get("pid") != pid):
        raise BenchmarkError("Unexpected startup report schema, clock or PID")
    stamps = report.get("timestamps_ns", {})
    if not isinstance(stamps, dict):
        raise BenchmarkError("Expected a phase timestamp object")
    values = [stamps.get(phase) for phase in PHASES]
    if not all(type(value) is int and value >= started_ns for value in values):
        raise BenchmarkError("Missing or invalid monotonic phase timestamp")
    if values[0] != min(values) or stamps["first_paint_ready"] > stamps["window_shown"]:
        raise BenchmarkError("Startup phases precede main entry or window shown precedes paint")
    # Page-load notifications may arrive before setup finishes. Sorting records
    # the real order rather than pretending the callbacks are always sequential.
    previous = started_ns
    intervals = {}
    for phase in sorted(PHASES, key=stamps.get):
        intervals[phase] = (stamps[phase] - previous) / 1_000_000
        previous = stamps[phase]
    return dict(timestamps_ns=stamps, phases_ms=intervals,
                elapsed_ms={p: (stamps[p] - started_ns) / 1_000_000 for p in PHASES},
                total_ms=(values[-1] - started_ns) / 1_000_000)


def startup_budgets(totals):
    """First launch is diagnostic; warm median and all-run p95 are gates."""
    if len(totals) < 2:
        raise BenchmarkError("At least two runs are required to measure warm startup")
    warm = summary(totals[1:])["p50"]
    all_p95 = summary(totals)["p95"]
    return [dict(metric="warm p50", actual=warm, limit=400, unit="ms", passed=warm <= 400),
            dict(metric="all-runs p95", actual=all_p95, limit=800, unit="ms", passed=all_p95 <= 800)]


def memory_budget(processes):
    """Gate physical footprint in decimal MB; RSS remains informational."""
    if not processes or not all(type(p.get("rss_bytes")) is int and p["rss_bytes"] >= 0
                               for p in processes):
        raise BenchmarkError("Expected valid per-process RSS samples")
    invalid = [p for p in processes if type(p.get("footprint_bytes")) is not int
               or p["footprint_bytes"] < 0]
    budget = dict(metric="total physical footprint", actual=None, limit=200,
                  unit="MB", passed=False)
    if invalid:
        details = "; ".join(f"PID {p.get('pid', 'unknown')}: "
                            f"{p.get('footprint_unavailable', 'missing or invalid sample')}"
                            for p in invalid)
        return dict(budget, error=f"Cannot measure physical footprint: {details}")
    total = sum(p["footprint_bytes"] for p in processes)
    return dict(budget, actual=total / 1_000_000, passed=total <= MEMORY_BUDGET_BYTES)


def print_budgets(budgets):
    """Print actionable failures and return a suitable command exit code."""
    for budget in budgets:
        if "error" in budget:
            print(f'FAIL: {budget["metric"]}: {budget["error"]}')
            continue
        status = "PASS" if budget["passed"] else "FAIL: budget exceeded"
        print(f'{status}: {budget["metric"]} {budget["actual"]:.2f} '
              f'{budget["unit"]} (limit {budget["limit"]} {budget["unit"]})')
    return int(any(not budget["passed"] for budget in budgets))

# SPDX-License-Identifier: Apache-2.0
"""Validate complete IPC spike samples before evaluating the WKWebView gate."""
import math
import statistics
from bench_metrics import BenchmarkError, summary

STAGES = ("invokeMs", "decodeMs", "workerDecodeMs", "totalMs")


def valid_timing(value):
    return type(value) in (float, int) and math.isfinite(value) and value >= 0


def timer_tolerance(report):
    timer = report.get("timer")
    if not isinstance(timer, dict) or type(timer.get("crossOriginIsolated")) is not bool:
        raise BenchmarkError("Missing or invalid page timer metadata")
    resolution = timer.get("resolutionMs")
    if not valid_timing(resolution) or resolution == 0 or not math.isfinite(2 * resolution):
        raise BenchmarkError("Missing or invalid page timer resolution")
    return 2 * resolution


def stage_summaries(report):
    """Diagnostic summaries survive invalid/partial runs; raw samples remain authoritative."""
    samples = report.get("samples") if isinstance(report, dict) else None
    if not isinstance(samples, list) or not samples:
        return {}
    output = {}
    for stage in STAGES:
        values = [sample.get(stage) if isinstance(sample, dict) else None for sample in samples]
        if all(valid_timing(value) for value in values):
            output[stage] = dict(summary(values), mean=statistics.fmean(values))
    return output


def ipc_metrics(report, pid):
    """Never accept partial runs, cache hits, nonfinite timings or wrong workloads."""
    if not isinstance(report, dict):
        raise BenchmarkError("IPC spike failed or returned an invalid report")
    if report.get("error") is not None:
        raise BenchmarkError(f"IPC spike failed: {report['error']}")
    expected = dict(schema_version=1, pid=pid, seed=42, rows=1_000_000, cols=20, tile_rows=256)
    if any(report.get(key) != value for key, value in expected.items()):
        raise BenchmarkError("Unexpected IPC workload/schema/PID")
    tolerance = timer_tolerance(report)
    samples = report.get("samples")
    if not isinstance(samples, list) or len(samples) != 500:
        raise BenchmarkError("Expected exactly 500 IPC samples")
    offsets = set()
    for sample in samples:
        if not isinstance(sample, dict):
            raise BenchmarkError("Invalid IPC sample")
        row = sample.get("rowStart")
        if type(row) is not int or not 0 <= row <= 1_000_000 - 256 or row in offsets:
            raise BenchmarkError("Invalid or repeated offset (cache hit)")
        offsets.add(row)
        values = [sample.get(stage) for stage in STAGES]
        if not all(valid_timing(value) for value in values):
            raise BenchmarkError("Invalid IPC timing")
        excess = sample["invokeMs"] + sample["decodeMs"] - sample["totalMs"]
        if excess > tolerance and not math.isclose(excess, tolerance, rel_tol=1e-12, abs_tol=1e-12):
            raise BenchmarkError("Total timing excludes a stage")
    stages = stage_summaries(report)
    actual = stages["totalMs"]["p95"]
    budget = dict(metric="tile round-trip p95", actual=actual, limit=3, unit="ms", passed=actual <= 3)
    return stages, [budget]


def encode_budget(estimates):
    """Gate the upper 95% confidence bound of Criterion's mean, in nanoseconds."""
    try:
        actual = estimates["mean"]["confidence_interval"]["upper_bound"]
    except (KeyError, TypeError) as error:
        raise BenchmarkError("Missing Criterion mean confidence interval") from error
    if type(actual) not in (int, float) or not math.isfinite(actual) or actual < 0:
        raise BenchmarkError("Invalid Criterion encode estimate")
    return dict(metric="256x20 encode mean upper bound", actual=actual / 1_000_000,
                limit=1, unit="ms", passed=actual <= 1_000_000)

# SPDX-License-Identifier: Apache-2.0
"""Monitor-open footprint (M25-09): a connected baseline against the same launch with the Monitor open.

Each launch runs the release app with an isolated home folder (no user profiles,
settings or WebKit data are read or written) and a spike mode that opens a
disposable in-memory SQLite connection; `monitor` also opens the Monitor
workspace and waits for its collector's first sample. The footprint is sampled
five seconds after the app reports readiness.
"""
from contextlib import contextmanager
from pathlib import Path
import statistics
import tempfile

from bench_common import ROOT, clock_ns, launch, wait_until
from bench_memory_metrics import memory_report, memory_sample
from bench_metrics import BenchmarkError

MODES = ("connected", "monitor")
MIB = 1024 * 1024
# ADR-0011: the Monitor adds at most 32 MiB of physical footprint across all processes.
INCREMENT_LIMIT_MIB = 32
READY_TIMEOUT_NS = 30_000_000_000
SETTLE_NS = 5_000_000_000


def isolated_environment(directory, mode):
    """HOME moves the Rust data folders; CFFIXED_USER_HOME moves WebKit's."""
    home = Path(directory) / "home"
    home.mkdir()
    return dict(HOME=str(home), CFFIXED_USER_HOME=str(home), OPENROWS_SPIKE=mode,
                OPENROWS_SPIKE_REPORT=str(Path(directory) / "spike.json"))


def wait_ready(process, progress):
    """Event-driven polling of the spike progress file; a reported error fails the run."""
    deadline = clock_ns() + READY_TIMEOUT_NS
    while True:
        lines = progress.read_text().splitlines() if progress.exists() else []
        errors = [line for line in lines if line.startswith("error:")]
        if errors:
            raise BenchmarkError(f"Memory spike failed: {errors[0]}")
        if any(line.startswith("memory-ready:") for line in lines):
            return
        if clock_ns() >= deadline:
            raise BenchmarkError("Memory spike was not ready within 30 seconds")
        wait_until(process, min(deadline, clock_ns() + 20_000_000))


@contextmanager
def scenario_launch(executable, mode):
    """The isolated home is removed with the temporary folder after cleanup."""
    with tempfile.TemporaryDirectory(prefix=".bench-monitor-", dir=ROOT) as directory:
        extra = isolated_environment(directory, mode)
        with launch(executable, extra) as (process, startup):
            wait_ready(process, Path(extra["OPENROWS_SPIKE_REPORT"]).with_suffix(".progress"))
            wait_until(process, clock_ns() + SETTLE_NS)
            yield process, startup


def increment_budget(connected, monitor):
    """Median Monitor-open total minus median connected total, in MiB."""
    if connected["median_total_footprint_bytes"] is None or monitor["median_total_footprint_bytes"] is None:
        return dict(metric="median Monitor footprint increment", actual=None, limit=INCREMENT_LIMIT_MIB,
                    unit="MiB", passed=False, error="A launch has no complete footprint")
    actual = (monitor["median_total_footprint_bytes"] - connected["median_total_footprint_bytes"]) / MIB
    return dict(metric="median Monitor footprint increment", actual=actual, limit=INCREMENT_LIMIT_MIB,
                unit="MiB", passed=actual <= INCREMENT_LIMIT_MIB)


def monitor_report(samples):
    """Connected launches keep the idle gates (NFR-PERF-02); the increment has its own."""
    by_mode = {mode: [sample for sample in samples if sample["mode"] == mode] for mode in MODES}
    if any(not values for values in by_mode.values()):
        raise BenchmarkError("Both connected and Monitor launches are required")
    connected, monitor = (memory_report(by_mode[mode]) for mode in MODES)
    budgets = [dict(budget, metric=f'connected {budget["metric"]}') for budget in connected["budgets"]]
    budgets.append(increment_budget(connected, monitor))
    return dict(connected=connected, monitor=monitor, budgets=budgets,
                increment_runs_mib=[(m["total_footprint_bytes"] - c["total_footprint_bytes"]) / MIB
                                    for c, m in zip(by_mode["connected"], by_mode["monitor"])
                                    if c["total_footprint_bytes"] is not None
                                    and m["total_footprint_bytes"] is not None])


def sample_scenarios(executable, runs, measure):
    """Alternate the modes so slow drift affects both alike."""
    samples = []
    for index in range(runs):
        for mode in MODES:
            with scenario_launch(executable, mode) as (process, startup):
                rows = measure(process)
            samples.append(dict(memory_sample(rows), mode=mode, run=index + 1, startup=startup))
            print(f"Run {index + 1} {mode}: total "
                  + (f'{samples[-1]["total_footprint_bytes"] / 1_000_000:.2f} MB'
                     if samples[-1]["total_footprint_bytes"] is not None else "unavailable"), flush=True)
    return samples


def print_monitor(report):
    for mode in MODES:
        values = report[mode]
        total, webview = values["median_total_footprint_bytes"], values["median_webview_footprint_bytes"]
        if total is not None:
            print(f"{mode:<10} median total {total / 1_000_000:.2f} MB; webview {webview / 1_000_000:.2f} MB")
    increments = report["increment_runs_mib"]
    if increments:
        print("Per-run Monitor increment (MiB): " + ", ".join(f"{value:.2f}" for value in increments)
              + f"; median {statistics.median(increments):.2f}")

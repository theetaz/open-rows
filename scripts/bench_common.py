# SPDX-License-Identifier: Apache-2.0
"""Effect boundaries shared by the macOS benchmarks; never build or open via CLI."""
import argparse
from contextlib import contextmanager
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import platform
import signal
import subprocess
import tempfile
import threading
import time

from bench_metrics import BenchmarkError, startup_sample
from bench_processes import Responsibility, app_processes, process_table, signal_if_same

ROOT = Path(__file__).resolve().parent.parent


def clock_ns():
    """Explicit clock ID also used by Rust (Python monotonic differs on macOS)."""
    return time.clock_gettime_ns(time.CLOCK_MONOTONIC)


def parser(description):
    """An executable override supports separate build targets without rebuilding."""
    result = argparse.ArgumentParser(description=description)
    result.add_argument("--executable", type=Path,
                        default=os.environ.get("OPENROWS_BENCH_EXECUTABLE"))
    return result


def release_executable(override):
    """Resolve Cargo's configured target without compiling anything."""
    if platform.system() != "Darwin":
        raise BenchmarkError("Benchmarks require a macOS GUI session; not Linux CI")
    if override:
        return Path(override).resolve(strict=True)
    result = subprocess.run(["cargo", "metadata", "--no-deps", "--locked", "--format-version", "1"],
                            cwd=ROOT, check=True, capture_output=True, text=True, timeout=30)
    target = Path(json.loads(result.stdout)["target_directory"])
    executable = target / "release/bundle/macos/OpenRows.app/Contents/MacOS/openrows-app"
    if not executable.is_file():
        raise BenchmarkError(f"Release app missing: {executable}. Run just build-app first")
    return executable


def wait_until(process, deadline_ns):
    """Timed event wait, with process-exit detection, never a shell sleep."""
    event = threading.Event()
    while clock_ns() < deadline_ns:
        if process.poll() is not None:
            raise BenchmarkError(f"App exited early (status {process.returncode})")
        event.wait(min(0.02, (deadline_ns - clock_ns()) / 1_000_000_000))


def read_report(process, report):
    """The app atomically renames a complete JSON report into this path."""
    deadline = clock_ns() + 30_000_000_000
    while not report.exists():
        if clock_ns() >= deadline:
            raise BenchmarkError("No complete startup report within 30 seconds")
        wait_until(process, min(deadline, clock_ns() + 10_000_000))
    return json.loads(report.read_text())


def remaining(owned):
    """Select only surviving identities, excluding PID reuse and zombies."""
    current = process_table()
    return [row for row in owned if row["pid"] in current
            and current[row["pid"]]["identity"] == row["identity"]]


def signal_group(pid, signum):
    """An exit racing with a fallback signal already completed cleanup."""
    try:
        os.killpg(pid, signum)
    except ProcessLookupError:
        pass


def stop_process(process, report, owned):
    """Request Tauri's normal exit; bound fallback signals and verify survivors."""
    report.with_suffix(".stop").touch()
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        signal_group(process.pid, signal.SIGTERM)
        try:
            process.wait(timeout=2)
        except subprocess.TimeoutExpired:
            signal_group(process.pid, signal.SIGKILL)
            process.wait(timeout=2)
    for signum in (signal.SIGTERM, signal.SIGKILL):
        current = process_table()
        for row in owned:
            signal_if_same(row, current, signum)
        deadline = clock_ns() + 2_000_000_000
        event = threading.Event()
        while remaining(owned) and clock_ns() < deadline:
            event.wait(0.02)
    survivors = remaining(owned)
    if survivors:
        raise BenchmarkError(f"Cleanup failed; surviving owned PIDs: {[p['pid'] for p in survivors]}")


@contextmanager
def launch(executable):
    """Cleanup runs on success, invalid report, timeout, failure and Ctrl-C."""
    owner = Responsibility()
    with tempfile.TemporaryDirectory(prefix=".bench-", dir=ROOT) as directory:
        report = Path(directory) / "startup.json"
        environment = dict(os.environ, OPENROWS_STARTUP_REPORT=str(report))
        started = clock_ns()
        process = subprocess.Popen([str(executable)], env=environment, start_new_session=True)
        owned = []
        try:
            data = read_report(process, report)
            sample = startup_sample(data, started, process.pid)
            yield process, sample
        finally:
            try:
                owned = app_processes(process_table(), process.pid, owner)
            finally:
                stop_process(process, report, owned)


def write_result(kind, payload):
    """Results are review artifacts; only temporary reports are removed."""
    directory = ROOT / "perf-results"
    directory.mkdir(exist_ok=True)
    timestamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S.%fZ")
    path = directory / f"{kind}-{timestamp}.json"
    payload.update(schema_version=1, recorded_at=timestamp, platform=platform.platform(),
                   machine=platform.machine())
    path.write_text(json.dumps(payload, indent=2) + "\n")
    print(f"Results: {path.relative_to(ROOT)}")


def run(command):
    """Concise command-line failures without obscuring cleanup exceptions."""
    try:
        return command()
    except KeyboardInterrupt:
        print("Benchmark interrupted; cleanup attempted")
        return 130
    except (BenchmarkError, OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"Benchmark failed: {error}")
        return 1

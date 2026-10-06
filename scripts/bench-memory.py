# SPDX-License-Identifier: Apache-2.0
"""Idle physical footprint across the app and its WebKit processes, five seconds after paint."""
from bench_common import clock_ns, launch, parser, release_executable, run, wait_until, write_result
from bench_metrics import BenchmarkError, print_budgets
from bench_memory_metrics import memory_sample, memory_report
from bench_processes import Responsibility, add_footprints, app_processes, process_table, require_webcontent


def measure(process):
    rows = app_processes(process_table(), process.pid, Responsibility())
    require_webcontent(rows)
    if process.pid not in {row["pid"] for row in rows} or process.poll() is not None:
        raise BenchmarkError("App exited during memory measurement")
    add_footprints(rows)
    return rows


def print_memory(rows):
    print(f'{"PID":>7} {"Process":<38} {"RSS MB":>10} {"Footprint MB":>14}')
    for row in rows:
        footprint = row["footprint_bytes"]
        value = f"{footprint / 1_000_000:.2f}" if footprint is not None else "unavailable"
        print(f'{row["pid"]:>7} {row["name"].split("/")[-1]:<38} '
              f'{row["rss_bytes"] / 1_000_000:>10.2f} {value:>14}')
    footprint = sum(row["footprint_bytes"] or 0 for row in rows)
    complete = all(row["footprint_bytes"] is not None for row in rows)
    print(f'TOTAL RSS (informational): {sum(row["rss_bytes"] for row in rows) / 1_000_000:.2f} MB')
    print(f'TOTAL footprint (run sample): {footprint / 1_000_000:.2f} MB' if complete else "TOTAL footprint (run sample): unavailable")


def sample_run(executable, index):
    with launch(executable) as (process, startup):
        deadline = startup["timestamps_ns"]["first_paint_ready"] + 5_000_000_000
        wait_until(process, deadline)
        sampled_ns = clock_ns()
        rows = measure(process)
    print(f"Run {index + 1}")
    print_memory(rows)
    sample = memory_sample(rows)
    webview = sample["webview_footprint_bytes"]
    print(f'Webview footprint: {webview / 1_000_000:.2f} MB' if webview is not None else "Webview footprint: unavailable")
    return dict(sample, sampled_ns=sampled_ns, startup=startup)


def main():
    arguments = parser(__doc__)
    arguments.add_argument("runs", type=int, nargs="?", default=3)
    options = arguments.parse_args()
    if options.runs < 1:
        raise BenchmarkError("At least one memory run is required")
    executable = release_executable(options.executable)
    samples = [sample_run(executable, index) for index in range(options.runs)]
    report = memory_report(samples)
    medians = report["process_median_footprint_bytes"]
    if medians is not None:
        print("Per-process median footprint (MB):")
        for name, value in medians.items():
            print(f"{name:<38} {value / 1_000_000:.2f}")
    write_result("memory", dict(report, executable=executable.name,
                               method="Sequential launches; first paint + 5 s; footprint -f bytes; "
                                      "ps RSS informational; descendants + process group + direct/inherited "
                                      "XPC responsibility with ps lstart >= root start; decimal MB"))
    return print_budgets(report["budgets"])


if __name__ == "__main__":
    raise SystemExit(run(main))

# SPDX-License-Identifier: Apache-2.0
"""Idle physical footprint across the app and its WebKit processes, five seconds after paint."""
from bench_common import clock_ns, launch, parser, release_executable, run, wait_until, write_result
from bench_metrics import BenchmarkError, memory_budget, print_budgets
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
    print(f'TOTAL footprint (gated): {footprint / 1_000_000:.2f} MB' if complete else "TOTAL footprint (gated): unavailable")


def main():
    options = parser(__doc__).parse_args()
    executable = release_executable(options.executable)
    with launch(executable) as (process, startup):
        deadline = startup["timestamps_ns"]["first_paint_ready"] + 5_000_000_000
        wait_until(process, deadline)
        sampled_ns = clock_ns()
        rows = measure(process)
    budget = memory_budget(rows)
    print_memory(rows)
    result_rows = [dict(row, name=row["name"].split("/")[-1]) for row in rows]
    write_result("memory", dict(executable=executable.name, processes=result_rows, budgets=[budget],
                                gated_metric="total_footprint_bytes",
                                total_rss_bytes=sum(row["rss_bytes"] for row in rows),
                                total_footprint_bytes=(sum(row["footprint_bytes"] for row in rows)
                                    if all(row["footprint_bytes"] is not None for row in rows) else None),
                                sampled_ns=sampled_ns, startup=startup,
                                method="footprint -f bytes (gate); ps RSS (informational); descendants + process group + direct/inherited XPC responsibility "
                                       "with ps lstart >= root start; decimal MB"))
    return print_budgets([budget])


if __name__ == "__main__":
    raise SystemExit(run(main))

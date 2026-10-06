# SPDX-License-Identifier: Apache-2.0
"""Run an already-built release IPC spike, print stages, and gate total p95."""
from bench_common import launch_ipc, parser, release_executable, run, write_result
from bench_ipc_metrics import ipc_metrics, stage_summaries
from bench_metrics import BenchmarkError, print_budgets
import subprocess


def main():
    options = parser(__doc__).parse_args()
    report, executable, budgets, error = None, None, [], None
    try:
        executable = release_executable(options.executable)
        with launch_ipc(executable) as (process, report):
            _, budgets = ipc_metrics(report, process.pid)
    except (BenchmarkError, OSError, ValueError, subprocess.SubprocessError) as caught:
        error = str(caught)
    stages = stage_summaries(report)
    payload = dict(report) if isinstance(report, dict) else dict(raw_report=report)
    verdict = "invalid" if error else "passed" if all(b["passed"] for b in budgets) else "budget_failed"
    payload.update(spike_error=payload.get("error"),
                   executable=executable.name if executable else None, stage_summary_ms=stages,
                   budgets=budgets, verdict=verdict, error=error)
    write_result("ipc", payload)
    print_stages(stages, payload.get("timer"))
    if error:
        print(f"Benchmark failed: {error}")
        return 1
    return print_budgets(budgets)


def print_stages(stages, timer):
    print(f"Page timer resolution: {timer.get('resolutionMs')} ms; "
          f"crossOriginIsolated: {timer.get('crossOriginIsolated')}"
          if isinstance(timer, dict) else "Page timer resolution: unavailable")
    print("Stage                         p50      p95      max     mean ms")
    for stage, values in stages.items():
        print(f'{stage:<25}' + ''.join(f'{values[key]:>9.3f}' for key in ("p50", "p95", "max", "mean")))


if __name__ == "__main__":
    raise SystemExit(run(main))

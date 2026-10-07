# SPDX-License-Identifier: Apache-2.0
"""Measure CodeMirror keystroke-to-paint on 5000 SQL lines in the release WKWebView."""
import json
import subprocess
from bench_common import launch_ipc, parser, release_executable, run, write_result
from bench_editor_metrics import BUDGET_MS, editor_budgets, editor_metrics, longest_keystrokes
from bench_metrics import BenchmarkError, print_budgets

METHOD = ("Gated keystrokeToPaintMs = scheduling delay + input handling to the CodeMirror document "
          "update + the first rendering update after it (rAF frame start to a task posted from that rAF). "
          "Only the idle wait from the update to the frame start (vsyncWaitMs) is excluded; wallMs keeps it. "
          f"p95 is nearest rank over all 240 keystrokes per configuration; exactly {BUDGET_MS} ms passes.")


def main():
    options = parser(__doc__).parse_args()
    print("Estimated duration: ~40 seconds (two configurations x 240 keystrokes); "
          "130-second report deadline. Keep the window visible.", flush=True)
    payload = dict(report=None, metrics={}, budgets=[], method=METHOD)
    error, executable = None, None
    try:
        executable = release_executable(options.executable)
        with launch_ipc(executable, mode="editor") as (process, report):
            payload["report"] = report
            payload["metrics"] = editor_metrics(report, process.pid)
        payload["budgets"] = editor_budgets(payload["metrics"])
    except (BenchmarkError, OSError, ValueError, subprocess.SubprocessError) as caught:
        error = str(caught)
    passed = payload["budgets"] and all(budget["passed"] for budget in payload["budgets"])
    payload.update(error=error, executable=executable.name if executable else None,
                   verdict="invalid" if error else "passed" if passed else "budget_failed")
    write_result("editor", payload)
    print_metrics(payload["report"], payload["metrics"])
    if error:
        print(f"Benchmark failed: {error}")
        return 1
    return print_budgets(payload["budgets"])


def print_metrics(report, metrics):
    timer = report.get("timer") if isinstance(report, dict) else None
    print(f"Page timer: {timer} (WKWebView commonly 1 ms; values are quantised)")
    print("Config  p50     p95     max     >16  refresh  queue95  input95  wait95  frame95  wall95")
    for config, values in metrics.items():
        phases = values["phase_p95_ms"]
        print(f"{config:<6}" + "".join(f"{values[key]:>8.2f}" for key in ("p50", "p95", "max"))
              + f"{values['over_budget']:>6}{values['refresh_interval_ms']:>9.2f}"
              + "".join(f"{phases[key]:>9.2f}" for key in ("queueMs", "inputMs", "vsyncWaitMs", "frameMs", "wallMs")))
        for position, p95 in values["position_p95_ms"].items():
            print(f"  {position:<7} keystroke-to-paint p95 {p95['keystrokeToPaintMs']:.2f} ms; wall p95 {p95['wallMs']:.2f} ms")
    print("Ten slowest keystrokes (raw samples retained in JSON):")
    for sample in longest_keystrokes(report):
        print(json.dumps(sample, sort_keys=True))


if __name__ == "__main__":
    raise SystemExit(run(main))

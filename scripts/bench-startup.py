# SPDX-License-Identifier: Apache-2.0
"""Repeatable release startup benchmark; OS disk caches are never purged."""
from bench_common import launch, parser, release_executable, run, write_result
from bench_metrics import PHASES, print_budgets, startup_budgets, summary


def main():
    arguments = parser(__doc__)
    arguments.add_argument("runs", type=int, nargs="?", default=10)
    options = arguments.parse_args()
    if options.runs < 2:
        arguments.error("runs must be at least 2 (first launch + warm launches)")
    executable = release_executable(options.executable)
    samples = []
    for index in range(options.runs):
        with launch(executable) as (_, sample):
            sample["launch"] = "first launch" if index == 0 else "warm"
            samples.append(sample)
        print(f'{index + 1:>3} {sample["launch"]:<12} {sample["total_ms"]:>8.2f} ms')
    phases = {phase: summary([s["phases_ms"][phase] for s in samples]) for phase in PHASES}
    phases["spawn → window shown"] = summary([s["total_ms"] for s in samples])
    print("Phase (interval ending at milestone)       min      p50      p95      max ms")
    for name, values in phases.items():
        print(f'{name:<39}' + "".join(f'{values[key]:>9.2f}' for key in ("min", "p50", "p95", "max")))
    budgets = startup_budgets([s["total_ms"] for s in samples])
    write_result("startup", dict(executable=executable.name, samples=samples,
                                 phase_summary_ms=phases, budgets=budgets,
                                 method="CLOCK_MONOTONIC spawn → window.show() return; first run not OS-cold"))
    return print_budgets(budgets)


if __name__ == "__main__":
    raise SystemExit(run(main))

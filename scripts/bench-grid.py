# SPDX-License-Identifier: Apache-2.0
"""Benchmark the accepted canvas grid with fresh app/scenario state per repeat."""
import subprocess
import json
from bench_common import launch_ipc, parser, release_executable, run, write_result
from bench_grid_metrics import grid_metrics, grid_budgets, longest_frames
from bench_metrics import BenchmarkError, print_budgets


def positive_repeats(value):
    repeats = int(value)
    if repeats < 1:
        raise ValueError('Grid repeats must be a positive integer')
    return repeats


def main():
    arguments = parser(__doc__)
    arguments.add_argument('repeats', nargs='?', type=positive_repeats, default=3)
    options = arguments.parse_args()
    print(f'Estimated duration: ~{38.5 * options.repeats:g} seconds sampling; '
          'each fresh launch has a 130-second report deadline', flush=True)
    payload = dict(repeats=options.repeats, reports=[], comparison={}, budgets=[], renderer='own')
    error, executable = None, None
    try:
        executable = release_executable(options.executable)
        for repeat in range(1, options.repeats + 1):
            print(f'Repeat {repeat}/{options.repeats}', flush=True)
            with launch_ipc(executable, mode='grid') as (process, report):
                payload['reports'].append(dict(repeat=repeat, report=report))
                metrics = grid_metrics(report, process.pid, repeat)
                payload['comparison'].update(metrics)
                print_repeat(repeat, report, metrics)
        payload['budgets'] = grid_budgets(payload['comparison'], options.repeats)
    except (BenchmarkError, OSError, ValueError, subprocess.SubprocessError) as caught:
        error = str(caught)
    payload.update(error=error, executable=executable.name if executable else None,
                   verdict='invalid' if error else 'passed' if all(b['passed'] for b in payload['budgets']) else 'budget_failed',
                   breakdown_method='Inclusive main-thread timing sum (may overlap); excludes worker CPU. '
                   'Best repeat per scenario must have zero frames >32ms; every repeat must have zero '
                   'frames >32ms with tracked work >=16ms. All raw samples retained; fresh app/cache per repeat.')
    write_result('grid', payload)
    if error:
        print(f'Benchmark failed: {error}')
        return 1
    return print_budgets(payload['budgets'])


def print_repeat(repeat, report, metrics):
    print(f'Repeat {repeat}; page timer: {report.get("timer")} (WKWebView commonly 1ms)')
    print('Scenario/repeat                     p50     p95     p99     max     >20   >32  own>=16  refresh  paint ms  tiles  hits  blanks')
    for key, values in metrics.items():
        print(f'{key:<34}' + ''.join(f'{values[field]:>8.2f}' for field in
              ('p50', 'p95', 'p99', 'max', 'over_20', 'over_32', 'own_cost_over_32',
               'refresh_interval_ms', 'mean_paint_ms', 'tiles', 'hits', 'blank_frames')))
    print(f'Repeat {repeat}: ten longest rAF intervals (inclusive tracked work; raw samples retained)')
    for frame in longest_frames(report):
        print(json.dumps(frame, sort_keys=True))


if __name__ == '__main__':
    raise SystemExit(run(main))

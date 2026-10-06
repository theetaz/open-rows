# SPDX-License-Identifier: Apache-2.0
"""Synthetic gates and command plumbing only; never launches the app."""
import copy
from contextlib import contextmanager
from pathlib import Path
import unittest
import tempfile
import json
import bench_common
from unittest.mock import Mock, patch
from bench_grid_metrics import DURATIONS, SCENARIOS, grid_metrics, grid_budgets, tracked_work, frame_summary, longest_frames
from bench_metrics import BenchmarkError
from test_bench_commands import load_command


def report():
    runs = []
    for option in ('own',):
        for scenario in SCENARIOS:
            duration = DURATIONS[scenario] + (16 if scenario == "jump" else 0)
            count = -int(-duration // 16)
            frames = [16] * (count - 1) + [duration - 16 * (count - 1)]
            runs.append(dict(option=option, scenario=scenario, frameMs=frames, paintMs=[1] * count,
                             durationMs=duration, cols=100 if scenario == 'horizontal' else 20,
                             tilesRequested=4000, cacheHits=50, blankCellFrames=2, startRow=0,
                             endRow=999977 if scenario == 'smooth' else 500000 if scenario == 'horizontal' else 501324 if scenario == 'flings' else 999000,
                             endColumn=99 if scenario == 'horizontal' else 19, paintMethod='test', maxScrollErrorPx=0))
    for sample in runs:
        add_breakdown(sample)
    return dict(schema_version=1, pid=42, seed=42, rows=1000000, cols=20, mode='grid',
                error=None, timer=dict(resolutionMs=1, crossOriginIsolated=False), grid=runs)


def add_breakdown(sample):
    timings = dict(scrollHandlerMs=0, reactCommitMs=0, canvasResizeMs=0, prepareMs=0,
                   tileApplyMs=0, tileConvertMs=0, workerConvertMs=0, driverMs=0)
    counts = dict(canvasResizeEvents=0, tileAllocations=0, arrayAllocations=0, cellAllocations=0)
    sample['frameBreakdown'] = [dict(timings, **counts, index=i, frameMs=f, paintMs=sample['paintMs'][i], step='preview')
                                for i, f in enumerate(sample['frameMs'])]
    sample['longestFrames'] = sorted(sample['frameBreakdown'], key=lambda item: -item['frameMs'])[:10]


def delay(sample, frame_ms, paint_ms=1):
    sample['durationMs'] += frame_ms - sample['frameMs'][0]
    sample['frameMs'][0] = frame_ms
    sample['paintMs'][0] = paint_ms
    add_breakdown(sample)


class GridTests(unittest.TestCase):
    def test_dropped_frames_and_measured_refresh_ignore_timer_quantisation(self):
        metrics = frame_summary([16, 17, 17, 20, 20.001, 32, 32.001])
        self.assertEqual(metrics['over_20'], 3)
        self.assertEqual(metrics['over_32'], 1)
        self.assertEqual(metrics['refresh_interval_ms'], 20)
        self.assertNotIn('over_16_7', metrics)

    def test_breakdown_and_top_ten_fail_closed(self):
        original = report()
        for field, value in (('frameMs', 17), ('paintMs', 2), ('index', 1), ('step', ''),
                             ('driverMs', float('nan')), ('tileAllocations', -1), ('arrayAllocations', 1.5)):
            data = copy.deepcopy(original)
            data['grid'][0]['frameBreakdown'][0][field] = value
            with self.subTest(field=field), self.assertRaises(BenchmarkError):
                grid_metrics(data, 42)
        data = copy.deepcopy(original)
        data['grid'][0]['longestFrames'] = []
        with self.assertRaisesRegex(BenchmarkError, 'Top ten'):
            grid_metrics(data, 42)
        peaks = longest_frames(original)
        self.assertEqual(len(peaks), 10)
        self.assertIn('scenario', peaks[0])

    def test_gate_includes_every_frame_not_a_percentile(self):
        for value, passed in ((32, True), (32.001, False)):
            data = report()
            sample = data['grid'][1]
            sample['frameMs'][0] = value
            sample['durationMs'] += value - 16
            add_breakdown(sample)
            comparison = grid_metrics(data, 42)
            budgets = grid_budgets(comparison, 1)
            self.assertEqual(budgets[1]['passed'], passed)
            self.assertEqual(comparison['own/flings/repeat-1']['max'], value)
            self.assertEqual(comparison['own/flings/repeat-1']['mean_paint_ms'], 1)

    def test_best_repeat_is_chosen_separately_for_each_scenario(self):
        metrics = {}
        for repeat in range(1, 4):
            data = report()
            for index in range(3):
                if index != repeat - 1:
                    delay(data['grid'][index], 40)
            metrics.update(grid_metrics(data, 42, repeat))
        budgets = grid_budgets(metrics, 3)
        self.assertTrue(all(item['passed'] for item in budgets))
        self.assertEqual(len(budgets), 16)
        for index, budget in enumerate(budgets[:3]):
            self.assertIn(f'repeat-{index + 1}', budget['metric'])

    def test_all_noisy_repeats_fail_the_scenario_gate(self):
        metrics = {}
        for repeat in range(1, 4):
            data = report()
            delay(data['grid'][1], 40)
            metrics.update(grid_metrics(data, 42, repeat))
        self.assertFalse(grid_budgets(metrics, 3)[1]['passed'])

    def test_tracked_cost_in_any_repeat_fails_even_with_a_clean_best_repeat(self):
        for frame_ms, work_ms, failed in ((32, 16, False), (33, 15.999, False), (33, 16, True)):
            metrics = grid_metrics(report(), 42, 1)
            data = report()
            delay(data['grid'][1], frame_ms, work_ms)
            metrics.update(grid_metrics(data, 42, 2))
            budgets = grid_budgets(metrics, 2)
            with self.subTest(frame=frame_ms, work=work_ms):
                self.assertTrue(budgets[1]['passed'])
                self.assertEqual(any(not item['passed'] for item in budgets), failed)
                self.assertEqual(metrics['own/flings/repeat-2']['own_cost_over_32'], int(failed))

    def test_tracked_work_sums_main_thread_categories_but_excludes_worker_and_counts(self):
        frame = report()['grid'][0]['frameBreakdown'][0]
        frame.update(paintMs=4, scrollHandlerMs=4, tileApplyMs=4, driverMs=4,
                     workerConvertMs=200, cellAllocations=5120)
        self.assertEqual(tracked_work(frame), 16)
        data = report()
        delay(data['grid'][0], 40)
        data['grid'][0]['frameBreakdown'][0]['workerConvertMs'] = 200
        self.assertTrue(all(item['passed'] for item in grid_budgets(grid_metrics(data, 42), 1)[4:]))

    def test_missing_or_unexpected_repeats_fail_closed(self):
        metrics = grid_metrics(report(), 42)
        for repeats in (0, 2):
            with self.subTest(repeats=repeats), self.assertRaises(BenchmarkError):
                grid_budgets(metrics, repeats)
        with self.assertRaises(BenchmarkError):
            grid_budgets(dict(metrics, extra=metrics['own/smooth/repeat-1']), 1)

    def test_rejects_partial_bad_timings_wrong_workload_and_endpoints(self):
        original = report()
        for key, value in (('cols', 100), ('durationMs', 4_333_000), ('endRow', 100), ('durationMs', 1), ('tilesRequested', 0),
                           ('maxScrollErrorPx', 1001), ('cacheHits', -1), ('blankCellFrames', 9999999), ('paintMethod', None),
                           ('frameMs', [float('nan')]), ('paintMs', [])):
            data = copy.deepcopy(original)
            data['grid'][0][key] = value
            with self.subTest(key=key), self.assertRaises(BenchmarkError):
                grid_metrics(data, 42)
        for data in (None, dict(original, error='failed'), dict(original, grid=[]), dict(original, pid=43)):
            with self.assertRaises(BenchmarkError):
                grid_metrics(data, 42)
        duplicate = copy.deepcopy(original)
        duplicate['grid'][1] = duplicate['grid'][0]
        with self.assertRaisesRegex(BenchmarkError, 'Repeated'):
            grid_metrics(duplicate, 42)

    def test_command_prints_all_repeats_and_gates_without_launching(self):
        command = load_command('bench-grid')
        data = report()
        delay(data['grid'][1], 33)
        @contextmanager
        def fake_launch(executable, mode):
            self.assertEqual(mode, 'grid')
            yield Mock(pid=42), data
        with patch('sys.argv', ['bench-grid.py']), patch.object(command, 'launch_ipc', fake_launch), \
             patch.object(command, 'release_executable', return_value=Path('fake')), \
             patch.object(command, 'write_result') as write, patch('builtins.print') as output:
            self.assertEqual(command.main(), 1)
            self.assertEqual(write.call_args.args[0], 'grid')
            payload = write.call_args.args[1]
            self.assertEqual(payload['verdict'], 'budget_failed')
            self.assertEqual(len(payload['reports']), 3)
            self.assertEqual(len(payload['comparison']), 12)
            self.assertIn('p99', str(output.call_args_list))
            self.assertIn('Estimated duration', str(output.call_args_list))
            for repeat in range(1, 4):
                self.assertIn(f'own/flings/repeat-{repeat}', str(output.call_args_list))

    def test_command_launches_fresh_repeats_and_accepts_environmental_noise(self):
        command = load_command('bench-grid')
        lifecycle = []
        @contextmanager
        def fake_launch(executable, mode):
            repeat = len(lifecycle) // 2 + 1
            lifecycle.append(f'launch-{repeat}')
            data = report()
            if repeat == 1:
                delay(data['grid'][0], 40)
            try:
                yield Mock(pid=42), data
            finally:
                lifecycle.append(f'cleanup-{repeat}')
        with patch('sys.argv', ['bench-grid.py', '2']), patch.object(command, 'launch_ipc', fake_launch), \
             patch.object(command, 'release_executable', return_value=Path('fake')), \
             patch.object(command, 'write_result') as write, patch('builtins.print'):
            self.assertEqual(command.main(), 0)
            payload = write.call_args.args[1]
            self.assertEqual(payload['verdict'], 'passed')
            self.assertIn('repeat-2', payload['budgets'][0]['metric'])
            self.assertEqual(len(payload['reports']), 2)
            self.assertEqual(lifecycle, ['launch-1', 'cleanup-1', 'launch-2', 'cleanup-2'])

    def test_command_honours_repeat_count_and_preserves_partial_invalid_report(self):
        command = load_command('bench-grid')
        @contextmanager
        def fake_launch(executable, mode):
            yield Mock(pid=42), dict(report(), grid=[])
        with patch('sys.argv', ['bench-grid.py', '2']), patch.object(command, 'launch_ipc', fake_launch), \
             patch.object(command, 'release_executable', return_value=Path('fake')), \
             patch.object(command, 'write_result') as write, patch('builtins.print'):
            self.assertEqual(command.main(), 1)
            self.assertEqual(write.call_args.args[1]['repeats'], 2)
            self.assertEqual(write.call_args.args[1]['verdict'], 'invalid')
            self.assertEqual(len(write.call_args.args[1]['reports']), 1)
        for value in ('0', '-1', 'oops'):
            with self.subTest(value=value), self.assertRaises(ValueError):
                command.positive_repeats(value)

    def test_progress_is_streamed_once_including_completion(self):
        with tempfile.TemporaryDirectory(dir=bench_common.ROOT) as directory:
            path = Path(directory) / 'grid.json'
            progress = path.with_suffix('.progress')
            progress.write_text('own/smooth started\n')
            def finish(_):
                progress.write_text('own/smooth started\nown/smooth complete\n')
                path.write_text(json.dumps(dict(done=True)))
            event = Mock()
            event.wait.side_effect = finish
            with patch.object(bench_common.threading, 'Event', return_value=event), patch('builtins.print') as output:
                data = bench_common.read_ipc_report(Mock(poll=lambda: None), path, timeout=130, progress=True)
            self.assertEqual(data, dict(done=True))
            self.assertEqual([call.args[0] for call in output.call_args_list], ['own/smooth started', 'own/smooth complete'])

    def test_native_report_deadline_reserves_launch_and_cleanup_time(self):
        root = dict(pid=42, group=42, parent=1, name='openrows-app', identity='root', started_at=100)
        process = Mock(pid=42, returncode=0)
        with patch.object(bench_common, 'Responsibility', return_value=lambda _: 7), \
             patch.object(bench_common, 'process_table', return_value={42: root}), \
             patch.object(bench_common.subprocess, 'Popen', return_value=process), \
             patch.object(bench_common, 'read_ipc_report', return_value=report()) as read, \
             patch.object(bench_common, 'stop_process'):
            with bench_common.launch_ipc(Path('fake'), mode='grid'):
                pass
            self.assertEqual(read.call_args.kwargs, dict(timeout=130, progress=True))

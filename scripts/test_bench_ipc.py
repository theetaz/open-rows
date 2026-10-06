# SPDX-License-Identifier: Apache-2.0
"""Synthetic IPC gates and command runs; no native app launches."""
from contextlib import contextmanager
import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock, patch

import bench_common
from bench_ipc_metrics import ipc_metrics
from bench_metrics import BenchmarkError
from test_bench_commands import load_command


def report(total=3):
    return dict(schema_version=1, pid=42, seed=42, rows=1_000_000, cols=20, tile_rows=256,
                timer=dict(resolutionMs=0.1, crossOriginIsolated=False),
                error=None, samples=[dict(rowStart=i * 997, invokeMs=1, decodeMs=0.5,
                                         workerDecodeMs=0.3, totalMs=total)
                                     for i in range(500)])


class IpcTests(unittest.TestCase):
    def test_quantised_stage_means_in_table_and_json(self):
        data = report(0)
        data['timer']['resolutionMs'] = 1
        for index, sample in enumerate(data['samples']):
            sample.update(invokeMs=int(index < 100), decodeMs=int(index < 162),
                          workerDecodeMs=int(index < 50), totalMs=int(index < 262))
        stages, budgets = ipc_metrics(data, 42)
        self.assertTrue(budgets[0]['passed'])
        command = load_command('bench-ipc')
        with patch('builtins.print') as output:
            command.print_stages(stages, data['timer'])
        self.assertIn('mean', str(output.call_args_list))
        for stage, expected in zip(stages, (0.2, 0.324, 0.1, 0.524)):
            self.assertEqual(stages[stage]['mean'], expected)
            self.assertIn(f'{expected:.3f}', str(output.call_args_list))
        self.check_command(data, 'passed', None)

    def test_rounding_tolerance_is_twice_measured_resolution(self):
        for resolution in (0.005, 0.1, 1):
            data = report(3)
            data['timer']['resolutionMs'] = resolution
            data['samples'][0].update(invokeMs=2, decodeMs=1 + 2 * resolution)
            self.assertTrue(ipc_metrics(data, 42)[1][0]['passed'])
            data['samples'][0]['decodeMs'] += 0.00001
            with self.assertRaisesRegex(BenchmarkError, 'excludes a stage'):
                ipc_metrics(data, 42)

    def test_rejects_invalid_timer_metadata(self):
        for timer in (None, {}, dict(resolutionMs=0.1, crossOriginIsolated=1)):
            with self.assertRaises(BenchmarkError):
                ipc_metrics(dict(report(), timer=timer), 42)
        for resolution in (None, 0, -1, True, float('nan'), float('inf')):
            timer = dict(resolutionMs=resolution, crossOriginIsolated=False)
            with self.assertRaises(BenchmarkError):
                ipc_metrics(dict(report(), timer=timer), 42)

    def test_gate_boundary_and_nearest_rank(self):
        for total, passed in ((3, True), (3.001, False)):
            stages, budgets = ipc_metrics(report(total), 42)
            self.assertEqual(budgets[0]["passed"], passed)
            self.assertEqual(stages["totalMs"]["p95"], total)
        data = report(2)
        for sample in data["samples"][-25:]:
            sample["totalMs"] = 10
        self.assertTrue(ipc_metrics(data, 42)[1][0]["passed"])
        data["samples"][-26]["totalMs"] = 10
        self.assertFalse(ipc_metrics(data, 42)[1][0]["passed"])

    def test_rejects_invalid_and_partial_reports(self):
        bad = [None, dict(report(), error="decode failed"), dict(report(), pid=43),
               dict(report(), samples=[]), dict(report(), cols=100)]
        for key, value in (("invokeMs", float("nan")), ("decodeMs", -1),
                           ("workerDecodeMs", float("inf")), ("invokeMs", True),
                           ("totalMs", 1), ("rowStart", -1), ("rowStart", True)):
            data = copy.deepcopy(report())
            data["samples"][0][key] = value
            bad.append(data)
        duplicate = report()
        duplicate["samples"][1]["rowStart"] = 0
        bad.append(duplicate)
        for data in bad:
            with self.subTest(data=str(data)[:80]), self.assertRaises(BenchmarkError):
                ipc_metrics(data, 42)

    def test_command_table_json_and_failure_status(self):
        self.check_command(report(3.1), 'budget_failed', None)

    def test_validation_failure_preserves_raw_samples_and_summaries(self):
        self.check_command(report(1), 'invalid', 'Total timing excludes a stage')

    def test_spike_failure_preserves_partial_samples(self):
        self.check_command(dict(report(), error='worker failed', samples=report()['samples'][:1]),
                           'invalid', 'IPC spike failed')

    def test_invalid_report_still_writes_json(self):
        for data in (None, [], 'broken'):
            self.check_command(data, 'invalid', 'invalid report')

    def test_launch_failure_still_writes_json(self):
        command = load_command('bench-ipc')
        with tempfile.TemporaryDirectory(dir=bench_common.ROOT) as directory:
            with patch.object(bench_common, 'ROOT', Path(directory)), \
                 patch('sys.argv', ['bench-ipc.py']), \
                 patch.object(command, 'release_executable', side_effect=BenchmarkError('missing app')), \
                 patch('builtins.print'):
                self.assertEqual(command.main(), 1)
                path, = (Path(directory) / 'perf-results').glob('ipc-*.json')
                result = json.loads(path.read_text())
                self.assertEqual(result['verdict'], 'invalid')
                self.assertEqual(result['error'], 'missing app')

    def test_cleanup_failure_preserves_collected_samples(self):
        self.check_command(report(), 'invalid', 'cleanup failed', cleanup_error=True)

    def check_command(self, data, verdict, error, cleanup_error=False):
        command = load_command("bench-ipc")
        @contextmanager
        def fake_launch(_):
            yield Mock(pid=42), data
            if cleanup_error:
                raise BenchmarkError('cleanup failed')
        with tempfile.TemporaryDirectory(dir=bench_common.ROOT) as directory:
            with patch.object(bench_common, "ROOT", Path(directory)), \
                 patch("sys.argv", ["bench-ipc.py"]), \
                 patch.object(command, "release_executable", return_value=Path("fake")), \
                 patch.object(command, "launch_ipc", fake_launch), patch("builtins.print") as output:
                self.assertEqual(command.main(), int(verdict != "passed"))
                path, = (Path(directory) / "perf-results").glob("ipc-*.json")
                result = json.loads(path.read_text())
                if isinstance(data, dict):
                    self.assertEqual(result['samples'], data['samples'])
                    self.assertEqual(result['timer'], data['timer'])
                    self.assertEqual(result['spike_error'], data['error'])
                    for stage, values in result['stage_summary_ms'].items():
                        timings = [sample[stage] for sample in data['samples']]
                        self.assertAlmostEqual(values['mean'], sum(timings) / len(timings))
                else:
                    self.assertEqual(result['raw_report'], data)
                self.assertEqual(result['verdict'], verdict)
                if error:
                    self.assertIn(error, result['error'])
                else:
                    self.assertEqual(result['budgets'][0]['passed'], verdict == 'passed')
                self.assertIn("p95", str(output.call_args_list))
                self.assertIn('resolution', str(output.call_args_list))


class IpcLifecycleTests(unittest.TestCase):
    def check_launch(self, *, failed=False):
        root = dict(pid=42, group=42, parent=1, name="openrows-app", identity="root", started_at=100)
        webkit = dict(pid=43, group=1, parent=1, name="com.apple.WebKit.WebContent",
                      identity="web", started_at=101)
        process = Mock(pid=42, returncode=0)
        process.poll.return_value = 0
        observed = {}
        def fake_spawn(_, **kwargs):
            observed.update(kwargs)
            path = Path(kwargs["env"]["OPENROWS_SPIKE_REPORT"])
            observed["directory"] = path.parent
            path.write_text('{broken' if failed else json.dumps(report()))
            return process
        with patch.object(bench_common, "Responsibility", return_value=lambda _: 7), \
             patch.object(bench_common, "process_table", side_effect=[{42: root}, {43: webkit}]), \
             patch.object(bench_common.subprocess, "Popen", side_effect=fake_spawn), \
             patch.object(bench_common, "stop_process") as stop:
            if failed:
                with self.assertRaises(ValueError), bench_common.launch_ipc(Path("fake")):
                    self.fail("Invalid JSON must fail before yielding")
            else:
                with bench_common.launch_ipc(Path("fake")) as (_, data):
                    self.assertEqual(data["pid"], 42)
                process.wait.assert_called_once_with(timeout=5)
            self.assertEqual(observed["env"]["OPENROWS_SPIKE"], "ipc")
            self.assertNotIn("OPENROWS_STARTUP_REPORT", observed["env"])
            self.assertEqual({row["pid"] for row in stop.call_args.args[2]}, {42, 43})
            self.assertFalse(observed["directory"].exists())

    def test_self_exit_retains_xpc_ownership_and_removes_temporary_report(self):
        self.check_launch()

    def test_invalid_report_still_cleans_up(self):
        self.check_launch(failed=True)

    def test_harness_timeout_prints_progress_and_stderr_then_cleans_up(self):
        root = dict(pid=42, group=42, parent=1, name="openrows-app", identity="root", started_at=100)
        process = Mock(pid=42)
        observed = {}
        def fake_spawn(_, **kwargs):
            path = Path(kwargs["env"]["OPENROWS_SPIKE_REPORT"])
            path.with_suffix(".progress").write_text("worker-created: blob\n")
            kwargs["stderr"].write("worker load blocked\n")
            observed["directory"] = path.parent
            return process
        with patch.object(bench_common, "Responsibility", return_value=lambda _: 7), \
             patch.object(bench_common, "process_table", return_value={42: root}), \
             patch.object(bench_common.subprocess, "Popen", side_effect=fake_spawn), \
             patch.object(bench_common, "read_ipc_report", side_effect=BenchmarkError("No IPC report within 120 seconds")), \
             patch.object(bench_common, "stop_process") as stop, patch("builtins.print") as output:
            with self.assertRaisesRegex(BenchmarkError, "120 seconds"), bench_common.launch_ipc(Path("fake")):
                self.fail("Timeout must fail before yielding")
            messages = str(output.call_args_list)
            self.assertIn("worker-created", messages)
            self.assertIn("worker load blocked", messages)
            stop.assert_called_once()
            self.assertFalse(observed["directory"].exists())

    def test_report_wait_is_bounded_and_early_exit_is_reported(self):
        with tempfile.TemporaryDirectory(dir=bench_common.ROOT) as directory:
            path = Path(directory) / "ipc.json"
            process = Mock(returncode=1)
            process.poll.return_value = None
            with patch.object(bench_common, "clock_ns", side_effect=[0, 120_000_000_000]):
                with self.assertRaisesRegex(BenchmarkError, "120 seconds"):
                    bench_common.read_ipc_report(process, path)
            process.poll.return_value = 1
            with self.assertRaisesRegex(BenchmarkError, "exited without report"):
                bench_common.read_ipc_report(process, path)


class EncodeGateTests(unittest.TestCase):
    def test_upper_confidence_bound_gate(self):
        from bench_ipc_metrics import encode_budget
        for value, passed in ((1_000_000, True), (1_000_001, False), (11_623, True)):
            estimate = dict(mean=dict(confidence_interval=dict(upper_bound=value)))
            self.assertEqual(encode_budget(estimate)["passed"], passed)
        for value in (None, {}, dict(mean=None), dict(mean=dict(confidence_interval=dict(upper_bound=-1)))):
            with self.assertRaises(BenchmarkError):
                encode_budget(value)

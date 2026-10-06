# SPDX-License-Identifier: Apache-2.0
"""Exercise cleanup boundaries using fake processes, never a GUI application."""
from contextlib import ExitStack
import json
from pathlib import Path
import subprocess
import tempfile
import sys
import unittest
from unittest.mock import Mock, patch

import bench_common
from bench_metrics import BenchmarkError, PHASES
from bench_processes import process_table


class LifecycleTests(unittest.TestCase):
    def launch_patches(self, directory, data):
        stack = ExitStack()
        self.addCleanup(stack.close)
        process = Mock(pid=42)
        stack.enter_context(patch.object(bench_common, "ROOT", Path(directory)))
        stack.enter_context(patch.object(bench_common, "Responsibility"))
        stack.enter_context(patch.object(bench_common.subprocess, "Popen", return_value=process))
        stack.enter_context(patch.object(bench_common, "read_report", return_value=data))
        stack.enter_context(patch.object(bench_common, "clock_ns", return_value=0))
        stack.enter_context(patch.object(bench_common, "process_table", return_value={}))
        stop = stack.enter_context(patch.object(bench_common, "stop_process"))
        return process, stop

    def valid_report(self):
        return dict(schema_version=1, clock="CLOCK_MONOTONIC", pid=42,
                    timestamps_ns={p: i + 1 for i, p in enumerate(PHASES)})

    def test_success_removes_temp_reports_and_stops(self):
        with tempfile.TemporaryDirectory(dir=bench_common.ROOT) as directory:
            process, stop = self.launch_patches(directory, self.valid_report())
            with bench_common.launch(Path("fake")) as (actual, sample):
                self.assertIs(actual, process)
                self.assertGreater(sample["total_ms"], 0)
                self.assertEqual(len(list(Path(directory).iterdir())), 1)
            stop.assert_called_once()
            self.assertEqual(list(Path(directory).iterdir()), [])

    def test_invalid_report_and_interrupt_cleanup(self):
        for data, error in (({}, BenchmarkError), (self.valid_report(), KeyboardInterrupt)):
            with self.subTest(error=error), tempfile.TemporaryDirectory(dir=bench_common.ROOT) as directory:
                _, stop = self.launch_patches(directory, data)
                with self.assertRaises(error), bench_common.launch(Path("fake")):
                    raise KeyboardInterrupt
                stop.assert_called_once()
                self.assertEqual(list(Path(directory).iterdir()), [])
                self.doCleanups()

    def test_report_timeout_still_stops(self):
        with tempfile.TemporaryDirectory(dir=bench_common.ROOT) as directory:
            _, stop = self.launch_patches(directory, self.valid_report())
            with patch.object(bench_common, "read_report", side_effect=BenchmarkError("timeout")):
                with self.assertRaises(BenchmarkError), bench_common.launch(Path("fake")):
                    self.fail("must not yield")
            stop.assert_called_once()
            self.assertEqual(list(Path(directory).iterdir()), [])

    def test_clean_exit_requests_tauri_shutdown(self):
        with tempfile.TemporaryDirectory(dir=bench_common.ROOT) as directory:
            report = Path(directory) / "startup.json"
            process = Mock(pid=42)
            with patch.object(bench_common, "process_table", return_value={}), patch.object(bench_common.os, "killpg") as kill:
                bench_common.stop_process(process, report, [])
            self.assertTrue(report.with_suffix(".stop").exists())
            process.wait.assert_called_once_with(timeout=5)
            kill.assert_not_called()

    def test_hung_app_escalates_and_is_reaped(self):
        with tempfile.TemporaryDirectory(dir=bench_common.ROOT) as directory:
            process = Mock(pid=42)
            process.wait.side_effect = [subprocess.TimeoutExpired("fake", 5),
                                        subprocess.TimeoutExpired("fake", 2), 0]
            with patch.object(bench_common, "process_table", return_value={}), patch.object(bench_common.os, "killpg") as kill:
                bench_common.stop_process(process, Path(directory) / "report.json", [])
            self.assertEqual([call.args for call in kill.call_args_list], [(42, 15), (42, 9)])
            self.assertEqual(process.wait.call_count, 3)

    def test_ps_parser_preserves_names_and_converts_kib(self):
        output = ("10 1 10 2048 S Tue Oct 6 12:00:00 2026 /path with spaces/app\n"
                  "11 1 1 1024 S Wed Sep 30 12:00:00 2026 com.apple.WebKit.WebContent\n")
        with patch("bench_processes.subprocess.run", return_value=Mock(stdout=output)) as run:
            rows = process_table()
        row = rows[10]
        self.assertEqual(row["rss_bytes"], 2_097_152)
        self.assertEqual(row["name"], "/path with spaces/app")
        self.assertEqual(row["identity"], "Tue Oct 6 12:00:00 2026")
        self.assertIsInstance(row["started_at"], float)
        self.assertLess(rows[11]["started_at"], row["started_at"])
        self.assertEqual(run.call_args.kwargs["env"]["LC_ALL"], "C")

    def test_report_is_read_without_poll_delay(self):
        with tempfile.TemporaryDirectory(dir=bench_common.ROOT) as directory:
            report = Path(directory) / "startup.json"
            report.write_text(json.dumps(self.valid_report()))
            with patch.object(bench_common, "wait_until") as wait:
                self.assertEqual(bench_common.read_report(Mock(), report), self.valid_report())
                wait.assert_not_called()

    def test_settle_uses_deadline_and_detects_early_exit(self):
        with patch.object(bench_common, "clock_ns", return_value=10):
            process = Mock()
            bench_common.wait_until(process, 5)
            process.poll.assert_not_called()
            process.poll.return_value = 1
            with self.assertRaises(BenchmarkError):
                bench_common.wait_until(process, 20)

    def test_real_headless_process_is_terminated_and_reaped(self):
        code = "import signal; print('ready', flush=True); signal.pause()"
        process = subprocess.Popen([sys.executable, "-c", code], start_new_session=True,
                                   stdout=subprocess.PIPE, text=True)
        try:
            self.assertEqual(process.stdout.readline().strip(), "ready")
            original_wait = process.wait
            def bounded_wait(timeout):
                if timeout == 5:
                    raise subprocess.TimeoutExpired("fixture", timeout)
                return original_wait(timeout=timeout)
            with tempfile.TemporaryDirectory(dir=bench_common.ROOT) as directory:
                with patch.object(process, "wait", side_effect=bounded_wait), patch.object(bench_common, "process_table", return_value={}):
                    bench_common.stop_process(process, Path(directory) / "startup.json", [])
            self.assertEqual(process.returncode, -15)
        finally:
            if process.poll() is None:
                process.kill()
            process.wait(timeout=2)
            process.stdout.close()

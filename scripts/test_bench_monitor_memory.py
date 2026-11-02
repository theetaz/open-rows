# SPDX-License-Identifier: Apache-2.0
"""Monitor-open footprint gates, readiness and isolation without launching the app."""
from contextlib import contextmanager
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock, patch

import bench_monitor_memory as monitor
from bench_metrics import BenchmarkError
from test_bench_memory import sample

MB = 1_000_000


def run(mode, core, webcontent):
    return dict(sample(core, webcontent), mode=mode)


class MonitorReportTests(unittest.TestCase):
    def test_increment_is_the_difference_of_medians_in_mib(self):
        report = monitor.monitor_report([
            run("connected", 60 * MB, 80 * MB), run("monitor", 70 * MB, 95 * MB),
            run("connected", 61 * MB, 80 * MB), run("monitor", 71 * MB, 96 * MB),
            run("connected", 59 * MB, 80 * MB), run("monitor", 69 * MB, 94 * MB)])
        increment = report["budgets"][-1]
        self.assertAlmostEqual(increment["actual"], 25 * MB / monitor.MIB)
        self.assertTrue(increment["passed"])
        self.assertEqual([b["metric"] for b in report["budgets"][:2]],
                         ["connected median total physical footprint", "connected median webview physical footprint"])
        self.assertEqual(len(report["increment_runs_mib"]), 3)

    def test_increment_above_32_mib_fails_and_missing_modes_are_invalid(self):
        report = monitor.monitor_report([run("connected", 60 * MB, 80 * MB),
                                         run("monitor", 60 * MB, 80 * MB + 32 * monitor.MIB + 1)])
        self.assertFalse(report["budgets"][-1]["passed"])
        with self.assertRaises(BenchmarkError):
            monitor.monitor_report([run("connected", 1, 1)])

    def test_incomplete_footprint_fails_the_increment(self):
        report = monitor.monitor_report([run("connected", 1, None), run("monitor", 1, 1)])
        self.assertFalse(report["budgets"][-1]["passed"])
        self.assertIn("error", report["budgets"][-1])


class LaunchTests(unittest.TestCase):
    def test_environment_isolates_home_and_webkit_data(self):
        with tempfile.TemporaryDirectory() as directory:
            environment = monitor.isolated_environment(directory, "monitor")
            self.assertEqual(environment["HOME"], environment["CFFIXED_USER_HOME"])
            self.assertTrue(Path(environment["HOME"]).is_dir())
            self.assertEqual(environment["OPENROWS_SPIKE"], "monitor")
            self.assertTrue(environment["OPENROWS_SPIKE_REPORT"].startswith(directory))

    def test_readiness_and_spike_errors_come_from_the_progress_file(self):
        process = Mock()
        process.poll.return_value = None
        with tempfile.TemporaryDirectory() as directory:
            progress = Path(directory, "spike.progress")
            progress.write_text("memory-connected: main\nmemory-ready: monitor\n")
            monitor.wait_ready(process, progress)
            progress.write_text("memory-connected: main\nerror: Monitor first sample timed out\n")
            with self.assertRaisesRegex(BenchmarkError, "first sample"):
                monitor.wait_ready(process, progress)
            progress.unlink()
            with patch.object(monitor, "READY_TIMEOUT_NS", 0), self.assertRaisesRegex(BenchmarkError, "not ready"):
                monitor.wait_ready(process, progress)

    def test_modes_alternate_per_run(self):
        launched = []

        @contextmanager
        def fake_launch(executable, mode):
            launched.append(mode)
            yield Mock(pid=1), dict(total_ms=1)
        rows = [dict(name="openrows-app", rss_bytes=1, footprint_bytes=MB),
                dict(name="com.apple.WebKit.WebContent", rss_bytes=1, footprint_bytes=MB)]
        with patch.object(monitor, "scenario_launch", fake_launch), patch("builtins.print"):
            samples = monitor.sample_scenarios(Path("fake"), 2, lambda process: rows)
        self.assertEqual(launched, ["connected", "monitor", "connected", "monitor"])
        self.assertEqual([s["run"] for s in samples], [1, 1, 2, 2])


if __name__ == "__main__":
    unittest.main()

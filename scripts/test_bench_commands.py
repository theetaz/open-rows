# SPDX-License-Identifier: Apache-2.0
"""Synthetic command runs verify tables, JSON and failure exit statuses."""
from contextlib import contextmanager, ExitStack
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock, patch

import bench_common
from bench_metrics import PHASES


def load_command(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).parent / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class CommandTests(unittest.TestCase):
    def test_startup_command_table_json_and_failure(self):
        command = load_command("bench-startup")
        totals = iter([300, 401, 401])
        @contextmanager
        def fake_launch(_):
            total = next(totals)
            yield Mock(), dict(total_ms=total, phases_ms={p: total / 7 for p in PHASES})
        with tempfile.TemporaryDirectory(dir=bench_common.ROOT) as directory, ExitStack() as stack:
            stack.enter_context(patch.object(bench_common, "ROOT", Path(directory)))
            stack.enter_context(patch("sys.argv", ["bench-startup.py", "3"]))
            stack.enter_context(patch.object(command, "release_executable", return_value=Path("fake")))
            stack.enter_context(patch.object(command, "launch", fake_launch))
            output = stack.enter_context(patch("builtins.print"))
            self.assertEqual(command.main(), 1)
            paths = list((Path(directory) / "perf-results").glob("startup-*.json"))
            self.assertEqual(len(paths), 1)
            result = json.loads(paths[0].read_text())
            self.assertEqual([s["launch"] for s in result["samples"]], ["first launch", "warm", "warm"])
            self.assertFalse(result["budgets"][0]["passed"])
            self.assertEqual(set(result["phase_summary_ms"]["main_entry"]), {"min", "p50", "p95", "max"})
            self.assertTrue(any("p95" in str(call) for call in output.call_args_list))

    def test_memory_command_total_json_deadline_and_verdict(self):
        for footprint, status in ((74_670_000, 0), (150_000_001, 1), (None, 1)):
            with self.subTest(footprint=footprint):
                self.check_memory_command(footprint, status)

    def check_memory_command(self, footprint, status):
        command = load_command("bench-memory")
        @contextmanager
        def fake_launch(_):
            yield Mock(), dict(timestamps_ns={"first_paint_ready": 123})
        rows = [dict(pid=42, name="openrows-app", rss_bytes=100_000_000, footprint_bytes=50_000_000),
                dict(pid=43, name="com.apple.WebKit.WebContent", rss_bytes=106_290_000,
                     footprint_bytes=footprint)]
        with tempfile.TemporaryDirectory(dir=bench_common.ROOT) as directory, ExitStack() as stack:
            stack.enter_context(patch.object(bench_common, "ROOT", Path(directory)))
            stack.enter_context(patch("sys.argv", ["bench-memory.py"]))
            stack.enter_context(patch.object(command, "release_executable", return_value=Path("fake")))
            stack.enter_context(patch.object(command, "launch", fake_launch))
            stack.enter_context(patch.object(command, "measure", return_value=rows))
            wait = stack.enter_context(patch.object(command, "wait_until"))
            output = stack.enter_context(patch("builtins.print"))
            self.assertEqual(command.main(), status)
            self.assertEqual(wait.call_args.args[1], 5_000_000_123)
            paths = list((Path(directory) / "perf-results").glob("memory-*.json"))
            self.assertEqual(len(paths), 1)
            result = json.loads(paths[0].read_text())
            self.assertEqual(result["total_rss_bytes"], 206_290_000)
            total = None if footprint is None else 50_000_000 + footprint
            self.assertEqual(result["total_footprint_bytes"], total)
            self.assertEqual(result["gated_metric"], "total_footprint_bytes")
            self.assertEqual(result["budgets"][0]["passed"], status == 0)
            if footprint is None:
                self.assertIn("PID 43", result["budgets"][0]["error"])
                self.assertTrue(any("Cannot measure physical footprint" in str(call)
                                    for call in output.call_args_list))
            printed = "\n".join(str(call) for call in output.call_args_list)
            self.assertIn("TOTAL RSS (informational): 206.29 MB", printed)
            self.assertIn("TOTAL footprint (gated):", printed)
            self.assertIn("com.apple.WebKit.WebContent", printed)

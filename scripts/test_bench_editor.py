# SPDX-License-Identifier: Apache-2.0
"""Synthetic editor gates and command plumbing only; never launches the app."""
import copy
from contextlib import contextmanager
from pathlib import Path
import unittest
from unittest.mock import Mock, patch

import bench_common
from bench_editor_metrics import (CONFIGS, editor_budgets, editor_metrics, expected_steps,
                                  longest_keystrokes)
from bench_metrics import BenchmarkError
from test_bench_commands import load_command


def sample(index, position, kind, config, gated=6.0):
    """input 2 + frame 3 + queue (gated - 5); wall adds a 7 ms idle wait."""
    queue = gated - 5
    key = "a" if kind == "insert" else dict(sql="Backspace", vim="x")[config]
    return dict(index=index, position=position, kind=kind, key=key, queueMs=queue, inputMs=2,
                vsyncWaitMs=7, frameMs=3, wallMs=12, keystrokeToPaintMs=gated)


def report(pid=42):
    runs = [dict(config=config, lines=5000, documentChars=200_000, refreshIntervalMs=16.67,
                 durationMs=16_000, samples=[sample(i, position, kind, config)
                                              for i, (position, kind) in enumerate(expected_steps())])
            for config in CONFIGS]
    return dict(schema_version=1, pid=pid, mode="editor", error=None,
                timer=dict(resolutionMs=1, crossOriginIsolated=False), editor=runs)


def slow(data, config, count, gated):
    run = next(run for run in data["editor"] if run["config"] == config)
    for item in run["samples"][:count]:
        item.update(sample(item["index"], item["position"], item["kind"], config, gated))


class EditorTests(unittest.TestCase):
    def test_script_has_sixty_inserts_and_twenty_deletes_per_site(self):
        steps = expected_steps()
        self.assertEqual(len(steps), 240)
        self.assertEqual(steps[59], ("top", "insert"))
        self.assertEqual(steps[60], ("top", "delete"))
        self.assertEqual(steps[-1], ("end", "delete"))

    def test_p95_gate_is_exact_at_sixteen_ms_per_configuration(self):
        # Twelve of 240 samples are the nearest-rank p95 boundary (index 227 of sorted values).
        for gated, passed in ((16, True), (16.001, False)):
            data = report()
            slow(data, "vim", 13, gated)
            metrics = editor_metrics(data, 42)
            budgets = editor_budgets(metrics)
            with self.subTest(gated=gated):
                self.assertTrue(budgets[0]["passed"])
                self.assertEqual(budgets[1]["passed"], passed)
                self.assertEqual(metrics["vim"]["p95"], gated)

    def test_tail_beyond_p95_does_not_fail_but_is_reported(self):
        data = report()
        slow(data, "sql", 12, 40)
        metrics = editor_metrics(data, 42)
        self.assertTrue(all(budget["passed"] for budget in editor_budgets(metrics)))
        self.assertEqual(metrics["sql"]["over_budget"], 12)
        self.assertEqual(metrics["sql"]["max"], 40)
        self.assertEqual(longest_keystrokes(data)[0]["config"], "sql")
        self.assertEqual(len(longest_keystrokes(data)), 10)

    def test_idle_vsync_wait_is_reported_but_not_gated(self):
        metrics = editor_metrics(report(), 42)
        self.assertEqual(metrics["sql"]["p95"], 6)
        self.assertEqual(metrics["sql"]["phase_p95_ms"]["wallMs"], 12)
        self.assertEqual(metrics["sql"]["position_p95_ms"]["middle"]["vsyncWaitMs"], 7)

    def test_phases_must_add_up(self):
        for field, value in (("keystrokeToPaintMs", 5.5), ("wallMs", 11), ("vsyncWaitMs", 6)):
            data = report()
            data["editor"][0]["samples"][3][field] = value
            with self.subTest(field=field), self.assertRaisesRegex(BenchmarkError, "add up"):
                editor_metrics(data, 42)

    def test_rejects_wrong_workload_order_keys_and_timings(self):
        original = report()
        mutations = [
            lambda d: d["editor"][0].update(lines=4999),
            lambda d: d["editor"][0].update(documentChars=0),
            lambda d: d["editor"][0].update(refreshIntervalMs=0),
            lambda d: d["editor"][0].update(durationMs=float("nan")),
            lambda d: d["editor"][0]["samples"].pop(),
            lambda d: d["editor"][0]["samples"][5].update(index=6),
            lambda d: d["editor"][0]["samples"][5].update(position="end"),
            lambda d: d["editor"][0]["samples"][5].update(kind="delete"),
            lambda d: d["editor"][0]["samples"][5].update(key="ab"),
            lambda d: d["editor"][0]["samples"][70].update(key="x"),
            lambda d: d["editor"][1]["samples"][70].update(key="Backspace"),
            lambda d: d["editor"][0]["samples"][5].update(inputMs=-1),
            lambda d: d["editor"][0]["samples"][5].update(frameMs=None),
            lambda d: d["editor"].reverse(),
            lambda d: d["editor"].pop(),
            lambda d: d.update(mode="grid"),
            lambda d: d.update(pid=43),
            lambda d: d.update(error="Vim did not enter insert mode"),
            lambda d: d.update(timer=dict(resolutionMs=None, crossOriginIsolated=False)),
        ]
        for index, mutate in enumerate(mutations):
            data = copy.deepcopy(original)
            mutate(data)
            with self.subTest(mutation=index), self.assertRaises(BenchmarkError):
                editor_metrics(data, 42)
        for data in (None, [], dict(original, editor=None)):
            with self.assertRaises(BenchmarkError):
                editor_metrics(data, 42)
        with self.assertRaises(BenchmarkError):
            editor_budgets(dict(sql=editor_metrics(original, 42)["sql"]))

    def test_command_writes_json_prints_phases_and_fails_budget(self):
        command = load_command("bench-editor")
        data = report()
        slow(data, "sql", 20, 17)
        @contextmanager
        def fake_launch(executable, mode):
            self.assertEqual(mode, "editor")
            yield Mock(pid=42), data
        with patch("sys.argv", ["bench-editor.py"]), patch.object(command, "launch_ipc", fake_launch), \
             patch.object(command, "release_executable", return_value=Path("fake")), \
             patch.object(command, "write_result") as write, patch("builtins.print") as output:
            self.assertEqual(command.main(), 1)
        kind, payload = write.call_args.args
        self.assertEqual(kind, "editor")
        self.assertEqual(payload["verdict"], "budget_failed")
        self.assertIs(payload["report"], data)
        self.assertIn("vsyncWaitMs", payload["method"])
        printed = str(output.call_args_list)
        for text in ("resolutionMs", "wall95", "middle", "FAIL: budget exceeded: sql keystroke-to-paint p95", "Estimated duration"):
            self.assertIn(text, printed)

    def test_command_passes_and_keeps_invalid_partial_reports(self):
        command = load_command("bench-editor")
        for data, status, verdict in ((report(), 0, "passed"), (dict(report(), editor=[]), 1, "invalid")):
            @contextmanager
            def fake_launch(executable, mode, data=data):
                yield Mock(pid=42), data
            with patch("sys.argv", ["bench-editor.py"]), patch.object(command, "launch_ipc", fake_launch), \
                 patch.object(command, "release_executable", return_value=Path("fake")), \
                 patch.object(command, "write_result") as write, patch("builtins.print"):
                with self.subTest(verdict=verdict):
                    self.assertEqual(command.main(), status)
                    self.assertEqual(write.call_args.args[1]["verdict"], verdict)
                    self.assertIs(write.call_args.args[1]["report"], data)

    def test_editor_launch_streams_progress_with_the_long_deadline(self):
        root = dict(pid=42, group=42, parent=1, name="openrows-app", identity="root", started_at=100)
        process = Mock(pid=42, returncode=0)
        with patch.object(bench_common, "Responsibility", return_value=lambda _: 7), \
             patch.object(bench_common, "process_table", return_value={42: root}), \
             patch.object(bench_common.subprocess, "Popen", return_value=process) as popen, \
             patch.object(bench_common, "read_ipc_report", return_value=report()) as read, \
             patch.object(bench_common, "stop_process"):
            with bench_common.launch_ipc(Path("fake"), mode="editor"):
                pass
        self.assertEqual(read.call_args.kwargs, dict(timeout=130, progress=True))
        self.assertEqual(popen.call_args.kwargs["env"]["OPENROWS_SPIKE"], "editor")

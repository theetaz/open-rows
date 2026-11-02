# SPDX-License-Identifier: Apache-2.0
"""Synthetic completion popup reports prove the M1-21 gate; never launches the app."""
import copy
import unittest

from bench_completion_metrics import (BUDGET_MS, REPEATS, TRIGGERS, completion_budgets,
                                      completion_metrics, expected_steps)
from bench_metrics import BenchmarkError


def popup(index, kind, key, popup_ms=30.0):
    return dict(index=index, kind=kind, key=key, options=12, openMs=popup_ms - 4, paintMs=4,
                popupMs=popup_ms)


def completion_run():
    return dict(objects=50_000, activateDelayMs=100, refreshIntervalMs=16.67,
                samples=[popup(index, kind, key) for index, (kind, key) in enumerate(expected_steps())])


def slow(run, kind, count, popup_ms):
    for sample in [sample for sample in run["samples"] if sample["kind"] == kind][:count]:
        sample.update(popup(sample["index"], kind, sample["key"], popup_ms))


class CompletionTests(unittest.TestCase):
    def test_script_cycles_every_trigger_per_repeat(self):
        steps = expected_steps()
        self.assertEqual(len(steps), REPEATS * len(TRIGGERS))
        self.assertEqual(steps[0], ("keyword", "S"))
        self.assertEqual(steps[3], ("column", "."))
        self.assertEqual(steps[4], ("keyword", "S"))

    def test_p95_gate_is_exact_at_fifty_ms_per_kind(self):
        # Nearest-rank p95 of 25 samples is the second largest: one outlier, one boundary sample.
        for popup_ms, passed in ((BUDGET_MS, True), (BUDGET_MS + 0.001, False)):
            run = completion_run()
            slow(run, "table", 1, 200)
            for sample in [s for s in run["samples"] if s["kind"] == "table"][1:2]:
                sample.update(popup(sample["index"], "table", "o", popup_ms))
            metrics = completion_metrics(dict(completion=run))
            budgets = {row["metric"]: row["passed"] for row in completion_budgets(metrics)}
            with self.subTest(popup_ms=popup_ms):
                self.assertEqual(budgets["table completion popup p95"], passed)
                self.assertTrue(budgets["keyword completion popup p95"])
                self.assertEqual(metrics["kinds"]["table"]["over_budget"], 1 + (not passed))
                self.assertEqual(metrics["kinds"]["table"]["p95"], popup_ms)

    def test_reports_phases_delay_and_options(self):
        metrics = completion_metrics(dict(completion=completion_run()))
        self.assertEqual(metrics["activate_delay_ms"], 100)
        self.assertEqual(metrics["kinds"]["column"]["phase_p95_ms"], dict(openMs=26, paintMs=4, popupMs=30))
        self.assertEqual(metrics["kinds"]["keyword"]["options_min"], 12)

    def test_rejects_wrong_workload_order_and_timings(self):
        original = completion_run()
        mutations = [
            lambda r: r.update(objects=49_999),
            lambda r: r.update(activateDelayMs=None),
            lambda r: r.update(refreshIntervalMs=0),
            lambda r: r["samples"].pop(),
            lambda r: r["samples"][5].update(index=6),
            lambda r: r["samples"][5].update(kind="keyword"),
            lambda r: r["samples"][5].update(key="x"),
            lambda r: r["samples"][5].update(options=0),
            lambda r: r["samples"][5].update(openMs=-1),
            lambda r: r["samples"][5].update(popupMs=31),
        ]
        for index, mutate in enumerate(mutations):
            run = copy.deepcopy(original)
            mutate(run)
            with self.subTest(mutation=index), self.assertRaises(BenchmarkError):
                completion_metrics(dict(completion=run))
        for data in (None, {}, dict(completion=None)):
            with self.assertRaises(BenchmarkError):
                completion_metrics(data)
        with self.assertRaises(BenchmarkError):
            completion_budgets(dict(kinds={}))


if __name__ == "__main__":
    unittest.main()

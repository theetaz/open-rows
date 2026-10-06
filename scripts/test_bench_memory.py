# SPDX-License-Identifier: Apache-2.0
"""Synthetic memory gates include outliers, split-process totals and invalid runs."""
import unittest
from bench_metrics import BenchmarkError
from bench_memory_metrics import memory_report, memory_sample


def sample(core, webcontent, gpu=0, networking=0):
    names = ("openrows-app", "com.apple.WebKit.WebContent",
             "com.apple.WebKit.GPU", "com.apple.WebKit.Networking")
    return memory_sample([dict(name=name, rss_bytes=1, footprint_bytes=value)
                          for name, value in zip(names, (core, webcontent, gpu, networking))])


class MemoryTests(unittest.TestCase):
    def test_median_ignores_valid_outlier_and_includes_every_webview_process(self):
        report = memory_report([sample(60_000_000, 100_000_000, 30_000_000, 10_000_000),
                                sample(400_000_000, 400_000_000),
                                sample(20_000_000, 100_000_000, 20_000_000, 10_000_000)])
        self.assertEqual(report["median_total_footprint_bytes"], 200_000_000)
        self.assertEqual(report["median_webview_footprint_bytes"], 140_000_000)
        self.assertTrue(all(b["passed"] for b in report["budgets"]))
        self.assertEqual(report["process_median_footprint_bytes"]["openrows-app"], 60_000_000)

    def test_webview_fails_even_when_total_passes(self):
        budgets = memory_report([sample(20_000_000, 130_000_000, 10_000_001)] * 3)["budgets"]
        self.assertTrue(budgets[0]["passed"])
        self.assertFalse(budgets[1]["passed"])

    def test_total_fails_even_when_webview_passes(self):
        budgets = memory_report([sample(60_000_001, 140_000_000)] * 3)["budgets"]
        self.assertFalse(budgets[0]["passed"])
        self.assertTrue(budgets[1]["passed"])

    def test_even_median_sums_runs_before_statistics(self):
        report = memory_report([sample(100_000_000, 1), sample(1, 100_000_000)])
        self.assertEqual(report["median_total_footprint_bytes"], 100_000_001)

    def test_invalid_run_cannot_be_hidden_by_median(self):
        report = memory_report([sample(1, 1), sample(1, None), sample(1, 1)])
        self.assertTrue(all(not b["passed"] for b in report["budgets"]))
        self.assertIn("Run 2", report["budgets"][0]["error"])
        self.assertIsNone(report["process_median_footprint_bytes"])
        with self.assertRaises(BenchmarkError):
            memory_report([])

    def test_process_medians_sum_duplicate_names_and_count_absent_names_as_zero(self):
        first = memory_sample([
            dict(name="com.apple.WebKit.WebContent", rss_bytes=1, footprint_bytes=30),
            dict(name="com.apple.WebKit.WebContent", rss_bytes=1, footprint_bytes=50),
            dict(name="com.apple.WebKit.GPU", rss_bytes=1, footprint_bytes=20),
        ])
        second = memory_sample([
            dict(name="com.apple.WebKit.WebContent", rss_bytes=1, footprint_bytes=100),
        ])
        medians = memory_report([first, second])["process_median_footprint_bytes"]
        self.assertEqual(medians["com.apple.WebKit.WebContent"], 90)
        self.assertEqual(medians["com.apple.WebKit.GPU"], 10)

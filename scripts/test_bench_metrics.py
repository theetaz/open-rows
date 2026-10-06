# SPDX-License-Identifier: Apache-2.0
"""Synthetic samples prove gates without a GUI or reference hardware."""
import unittest
from unittest.mock import patch

from bench_metrics import (BenchmarkError, PHASES, memory_budget, print_budgets,
                           startup_budgets, startup_sample, summary)
from bench_processes import app_processes, footprint_bytes, require_webcontent, signal_if_same


def process(pid, parent=1, group=1, name="openrows-app"):
    return dict(pid=pid, parent=parent, group=group, name=name,
                identity="Tue Oct 6 12:00:00 2026", started_at=100, rss_bytes=1_000_000)


class BudgetTests(unittest.TestCase):
    def test_startup_boundaries_pass(self):
        budgets = startup_budgets([800, 400, 400])
        self.assertTrue(all(row["passed"] for row in budgets))
        self.assertEqual(budgets[0]["actual"], 400)

    def test_warm_budget_failure(self):
        self.assertFalse(startup_budgets([100, 400.01, 400.01])[0]["passed"])

    def test_tail_budget_failure(self):
        budgets = startup_budgets([801] + [100] * 9)
        self.assertFalse(budgets[1]["passed"])
        with patch("builtins.print") as output:
            self.assertEqual(print_budgets(budgets), 1)
            self.assertIn("budget exceeded", output.call_args_list[1].args[0])

    def test_invalid_samples_fail(self):
        for samples in ([], [1], [1, float("nan")], [1, -1]):
            with self.assertRaises(BenchmarkError):
                startup_budgets(samples)

    def test_statistics(self):
        self.assertEqual(summary([40, 10, 30, 20]),
                         dict(min=10, p50=25, p95=40, max=40))

    def test_memory_boundary_and_failure(self):
        self.assertTrue(memory_budget([dict(rss_bytes=300_000_000, footprint_bytes=200_000_000)])["passed"])
        budget = memory_budget([dict(rss_bytes=1, footprint_bytes=100_000_000),
                                dict(rss_bytes=1, footprint_bytes=100_000_001)])
        self.assertFalse(budget["passed"])
        with patch("builtins.print"):
            self.assertEqual(print_budgets([budget]), 1)

    def test_rss_over_limit_footprint_under_passes(self):
        budget = memory_budget([dict(rss_bytes=206_290_000, footprint_bytes=124_670_000)])
        self.assertTrue(budget["passed"])
        self.assertEqual(budget["actual"], 124.67)
        self.assertEqual(budget["metric"], "total physical footprint")

    def test_missing_or_invalid_footprint_fails(self):
        for sample in ({}, dict(footprint_bytes=None), dict(footprint_bytes=-1),
                       dict(footprint_bytes=1.5), dict(footprint_bytes=True)):
            rows = [dict(rss_bytes=1, footprint_bytes=1), dict(sample, pid=43, rss_bytes=1)]
            budget = memory_budget(rows)
            self.assertFalse(budget["passed"])
            self.assertIsNone(budget["actual"])
            self.assertIn("PID 43", budget["error"])
            with patch("builtins.print") as output:
                self.assertEqual(print_budgets([budget]), 1)
                self.assertIn("Cannot measure physical footprint", output.call_args.args[0])

    def test_invalid_memory(self):
        for rows in ([], [dict(rss_bytes=-1)], [dict(rss_bytes=1.5)]):
            with self.assertRaises(BenchmarkError):
                memory_budget(rows)


class ReportTests(unittest.TestCase):
    def report(self):
        return dict(schema_version=1, clock="CLOCK_MONOTONIC", pid=42,
                    timestamps_ns={name: (index + 1) * 1_000_000 for index, name in enumerate(PHASES)})

    def test_shared_clock_total_excludes_report_poll_delay(self):
        sample = startup_sample(self.report(), 0, 42)
        self.assertEqual(sample["total_ms"], 7)
        self.assertEqual(sum(sample["phases_ms"].values()), 7)
        self.assertEqual(sample["elapsed_ms"]["main_entry"], 1)

    def test_callback_order_is_not_fabricated(self):
        report = self.report()
        stamps = report["timestamps_ns"]
        stamps["setup_complete"], stamps["page_load_finished"] = (
            stamps["page_load_finished"], stamps["setup_complete"])
        self.assertEqual(sum(startup_sample(report, 0, 42)["phases_ms"].values()), 7)

    def test_late_page_load_does_not_extend_window_show_total(self):
        report = self.report()
        report["timestamps_ns"]["page_load_finished"] = 8_000_000
        sample = startup_sample(report, 0, 42)
        self.assertEqual(sample["total_ms"], 7)
        self.assertEqual(sample["elapsed_ms"]["page_load_finished"], 8)

    def test_malformed_report_is_explicit(self):
        for report in ([], dict(self.report(), timestamps_ns=[])):
            with self.assertRaises(BenchmarkError):
                startup_sample(report, 0, 42)

    def test_missing_phase_rejected(self):
        report = self.report()
        del report["timestamps_ns"]["window_shown"]
        with self.assertRaises(BenchmarkError):
            startup_sample(report, 0, 42)

    def test_wrong_pid_clock_and_time_rejected(self):
        for change in (dict(pid=99), dict(clock="wall"), dict(schema_version=2)):
            with self.assertRaises(BenchmarkError):
                startup_sample(dict(self.report(), **change), 0, 42)
        with self.assertRaises(BenchmarkError):
            startup_sample(self.report(), 2_000_000, 42)


class AttributionTests(unittest.TestCase):
    def test_inherited_host_responsibility_excludes_older_and_unrelated_webkit(self):
        rows = {10: process(10, group=10),
                12: dict(process(12, name="com.apple.WebKit.WebContent"), started_at=99),
                13: process(13, name="com.apple.WebKit.WebContent"),
                14: dict(process(14, name="com.apple.WebKit.GPU"), started_at=101),
                15: dict(process(15, name="com.apple.WebKit.Networking"), started_at=102),
                16: dict(process(16, name="com.apple.WebKit.WebContent"), started_at=103)}
        selected = app_processes(rows, 10, lambda pid: 768 if pid == 16 else 2880)
        self.assertEqual([row["pid"] for row in selected], [10, 13, 14, 15])
        require_webcontent(selected)

    def test_selected_owner_also_requires_start_time_cutoff(self):
        rows = {10: process(10, group=10), 11: process(11, parent=10),
                12: dict(process(12, name="com.apple.WebKit.WebContent"), started_at=99),
                13: process(13, name="com.apple.WebKit.WebContent")}
        selected = app_processes(rows, 10, lambda pid: 2880 if pid == 10 else 11)
        self.assertEqual([row["pid"] for row in selected], [10, 11, 13])

    def test_only_older_host_webcontent_cannot_certify_rss(self):
        rows = {10: process(10, group=10),
                12: dict(process(12, name="com.apple.WebKit.WebContent"), started_at=99)}
        with self.assertRaisesRegex(BenchmarkError, "No app-owned WebContent"):
            require_webcontent(app_processes(rows, 10, lambda _: 2880))

    def test_exited_root_does_not_attribute_host_processes(self):
        rows = {12: process(12, name="com.apple.WebKit.WebContent")}
        self.assertEqual(app_processes(rows, 10, lambda _: 2880), [])

    def test_launchd_webkit_owned_by_app_included(self):
        rows = {10: process(10, group=10), 11: process(11, parent=10),
                12: process(12, name="com.apple.WebKit.WebContent"),
                13: process(13, name="com.apple.WebKit.GPU"),
                14: process(14, name="com.apple.WebKit.Networking"),
                15: process(15, name="com.apple.WebKit.WebContent")}
        selected = app_processes(rows, 10, lambda pid: 99 if pid == 15 else 10)
        self.assertEqual([row["pid"] for row in selected], [10, 11, 12, 13, 14])
        require_webcontent(selected)

    def test_missing_webcontent_cannot_pass(self):
        with self.assertRaises(BenchmarkError):
            require_webcontent([process(10)])

    def test_footprint_byte_parser(self):
        self.assertEqual(footprint_bytes("app [10]: Footprint: 12,345 B (4096 bytes per page)"), 12345)
        with self.assertRaises(BenchmarkError):
            footprint_bytes("phys_footprint_peak: 42 MB")

    def test_pid_reuse_is_not_signalled(self):
        original = process(10)
        current = {10: dict(original, identity="different start")}
        with patch("bench_processes.os.kill") as kill:
            signal_if_same(original, current, 15)
            kill.assert_not_called()

# SPDX-License-Identifier: Apache-2.0
"""Synthetic schema_* gates; no Criterion run needed."""
import unittest

from bench_metrics import BenchmarkError
from bench_schema_metrics import SEARCH_BENCHES, mean_upper_budget, schema_budgets


def estimate(upper_ns):
    return dict(mean=dict(confidence_interval=dict(upper_bound=upper_ns)))


class SchemaGateTests(unittest.TestCase):
    def test_limits_are_inclusive_and_use_the_upper_bound(self):
        for upper_ns, passed in ((50_000_000, True), (50_000_001, False), (7_037_400, True)):
            self.assertEqual(mean_upper_budget("s", estimate(upper_ns), 50)["passed"], passed)

    def test_first_level_and_every_search_are_gated(self):
        seen = []

        def read(group, function):
            seen.append((group, function))
            return estimate(299_000_000 if group == "schema_first_level_50k" else 51_000_000)

        budgets = schema_budgets(read)
        self.assertEqual(len(budgets), 1 + len(SEARCH_BENCHES))
        self.assertTrue(budgets[0]["passed"])
        self.assertFalse(any(budget["passed"] for budget in budgets[1:]))
        self.assertEqual(seen[0], ("schema_first_level_50k", "snapshot_restore"))

    def test_invalid_criterion_output_fails_closed(self):
        for value in (None, {}, estimate(0), estimate(float("nan")), estimate("1")):
            with self.assertRaises(BenchmarkError):
                mean_upper_budget("s", value, 50)


if __name__ == "__main__":
    unittest.main()

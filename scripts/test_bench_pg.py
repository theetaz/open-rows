# SPDX-License-Identifier: Apache-2.0
"""Synthetic decode_pg_* gates; no Criterion run or database needed."""
import unittest

from bench_metrics import BenchmarkError
from bench_pg_metrics import first_batch_budget, throughput_budget


def estimate(upper_ns):
    return dict(mean=dict(confidence_interval=dict(upper_bound=upper_ns)))


def benchmark(cells):
    return dict(throughput=dict(Elements=cells))


class DecodeGateTests(unittest.TestCase):
    def test_throughput_uses_upper_bound_per_core(self):
        # 1 M cells in exactly 1 s is the boundary; slower fails.
        for upper_ns, passed in ((1_000_000_000, True), (1_000_000_001, False), (3_067_343, True)):
            budget = throughput_budget("decode_pg_int", estimate(upper_ns), benchmark(1_000_000))
            self.assertEqual(budget["passed"], passed)
        budget = throughput_budget("decode_pg_int", estimate(2_000_000), benchmark(1_000_000))
        self.assertAlmostEqual(budget["actual"], 500.0)
        self.assertEqual(budget["unit"], "M cells/s")

    def test_first_batch_limit_is_inclusive(self):
        for upper_ns, passed in ((5_000_000, True), (5_000_001, False), (43_606, True)):
            self.assertEqual(first_batch_budget(estimate(upper_ns))["passed"], passed)

    def test_invalid_criterion_output_fails_closed(self):
        for value in (None, {}, estimate(0), estimate(-1), estimate(float("nan")), estimate("1")):
            with self.assertRaises(BenchmarkError):
                first_batch_budget(value)
        for value in (None, {}, benchmark(0), benchmark(1.5), dict(throughput=dict(Bytes=10))):
            with self.assertRaises(BenchmarkError):
                throughput_budget("decode_pg_wide", estimate(1_000), value)


if __name__ == "__main__":
    unittest.main()

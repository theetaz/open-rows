# SPDX-License-Identifier: Apache-2.0
"""Synthetic Criterion trees prove the regression rule against the main baseline."""
import io
import json
from pathlib import Path
import tempfile
import unittest
from contextlib import redirect_stdout

from bench_compare import CompareError, compare, main, threshold


def estimate(root, benchmark, change, lower, upper):
    path = Path(root, benchmark, "change", "estimates.json")
    path.parent.mkdir(parents=True)
    interval = dict(confidence_level=0.95, lower_bound=lower, upper_bound=upper)
    path.write_text(json.dumps(dict(mean=dict(point_estimate=change, confidence_interval=interval))))


class CompareTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.root = self.directory.name

    def tearDown(self):
        self.directory.cleanup()

    def test_thresholds_follow_the_benchmark_group(self):
        self.assertEqual(threshold("sql_split_postgres/1mb"), 0.10)
        self.assertEqual(threshold("result_window_encode/256x20"), 0.05)

    def test_only_a_confident_regression_beyond_the_limit_fails(self):
        estimate(self.root, "result_window_encode/256x20", 0.08, 0.051, 0.11)
        estimate(self.root, "decode_int/20x200", 0.09, -0.01, 0.19)
        estimate(self.root, "sql_split_mysql/10kb", 0.12, 0.09, 0.15)
        rows = {row["benchmark"]: row for row in compare(self.root)}
        self.assertTrue(rows["result_window_encode/256x20"]["regressed"])
        self.assertFalse(rows["decode_int/20x200"]["regressed"])
        self.assertFalse(rows["sql_split_mysql/10kb"]["regressed"])
        with redirect_stdout(io.StringIO()) as output:
            self.assertEqual(main([self.root]), 1)
        self.assertIn("1 of 3 benchmarks regressed", output.getvalue())

    def test_improvements_pass(self):
        estimate(self.root, "schema_search_50k/exact", -0.2, -0.25, -0.15)
        with redirect_stdout(io.StringIO()):
            self.assertEqual(main([self.root]), 0)

    def test_missing_or_malformed_estimates_fail(self):
        with self.assertRaises(CompareError):
            compare(self.root)
        path = Path(self.root, "x", "change", "estimates.json")
        path.parent.mkdir(parents=True)
        path.write_text(json.dumps(dict(mean=dict(point_estimate=0.1))))
        with redirect_stdout(io.StringIO()) as output:
            self.assertEqual(main([self.root]), 1)
        self.assertIn("Malformed", output.getvalue())
        with redirect_stdout(io.StringIO()):
            self.assertEqual(main([]), 2)


if __name__ == "__main__":
    unittest.main()

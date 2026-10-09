# SPDX-License-Identifier: Apache-2.0
"""Peak RSS parsing uses platform units and refuses incomplete evidence."""
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("clickhouse_bench", Path(__file__).with_name("bench-clickhouse.py"))
benchmark = importlib.util.module_from_spec(spec)
spec.loader.exec_module(benchmark)


class PeakRssTests(unittest.TestCase):
    def test_darwin_bytes_and_linux_kib(self):
        self.assertEqual(benchmark.peak_rss("14893056  maximum resident set size", "darwin"), 14893056)
        self.assertEqual(benchmark.peak_rss("Maximum resident set size (kbytes): 1024", "linux"), 1048576)

    def test_missing_zero_and_wrong_units_are_rejected(self):
        for output in ["", "0 maximum resident set size", "Maximum resident set size (kbytes): 12"]:
            with self.assertRaises(RuntimeError):
                benchmark.peak_rss(output, "darwin")

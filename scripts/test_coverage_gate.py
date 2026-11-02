# SPDX-License-Identifier: Apache-2.0
"""Synthetic llvm-cov exports prove the coverage grouping and floor."""
import io
import json
import unittest
import unittest.mock
from contextlib import redirect_stdout

from coverage_gate import CoverageError, group_of, group_totals, main, print_gate


def entry(filename, count, covered):
    return dict(filename=filename, summary=dict(lines=dict(count=count, covered=covered)))


def export(*files):
    return dict(data=[dict(files=list(files))])


PASSING = export(
    entry("/repo/crates/openrows-core/src/actor.rs", 100, 90),
    entry("/repo/crates/openrows-sql/src/split.rs", 50, 45),
    entry("/repo/crates/openrows-driver-postgres/src/decode/scalar.rs", 40, 36),
    entry("/repo/crates/openrows-driver-mysql/src/values.rs", 10, 8),
)


class GroupTests(unittest.TestCase):
    def test_groups_own_their_sources_and_skip_tests_and_other_crates(self):
        self.assertEqual(group_of("/r/crates/openrows-core/src/manager/mod.rs"), "openrows-core")
        self.assertEqual(group_of("C:\\r\\crates\\openrows-sql\\src\\bind.rs"), "openrows-sql")
        self.assertEqual(group_of("/r/crates/openrows-driver-sqlite/src/decode.rs"), "value codecs")
        self.assertEqual(group_of("/r/crates/openrows-driver-mssql/src/decode/number.rs"), "value codecs")
        self.assertIsNone(group_of("/r/crates/openrows-driver-mysql/src/session.rs"))
        self.assertIsNone(group_of("/r/crates/openrows-driver-postgres/src/decode/array_tests.rs"))
        self.assertIsNone(group_of("/r/crates/openrows-driver-mysql/src/decode/tests/typed.rs"))
        self.assertIsNone(group_of("/r/crates/openrows-core/src/tests/script.rs"))
        self.assertIsNone(group_of("/r/crates/openrows-results/src/lib.rs"))

    def test_totals_are_line_weighted_per_group(self):
        totals = group_totals(PASSING)
        self.assertEqual(totals["value codecs"], dict(covered=44, lines=50, percent=88.0))
        self.assertEqual(totals["openrows-core"]["percent"], 90.0)


class GateTests(unittest.TestCase):
    def test_floor_is_inclusive_and_a_lower_group_fails(self):
        at_floor = group_totals(export(
            entry("/r/crates/openrows-core/src/a.rs", 10, 8),
            entry("/r/crates/openrows-sql/src/a.rs", 10, 8),
            entry("/r/crates/openrows-driver-sqlite/src/decode.rs", 10, 8)))
        below = dict(at_floor, **{"openrows-sql": dict(covered=7, lines=10, percent=79.99)})
        with redirect_stdout(io.StringIO()) as output:
            self.assertEqual(print_gate(at_floor), 0)
            self.assertEqual(print_gate(below), 1)
        self.assertIn("FAIL", output.getvalue())

    def test_missing_groups_and_malformed_reports_fail(self):
        with self.assertRaises(CoverageError):
            group_totals(export(entry("/r/crates/openrows-core/src/a.rs", 1, 1)))
        with self.assertRaises(CoverageError):
            group_totals(dict(data=[]))
        with self.assertRaises(CoverageError):
            group_totals(export(entry("/r/crates/openrows-core/src/a.rs", 1, 2)))

    def test_main_reads_a_report_file(self):
        with unittest.mock.patch("pathlib.Path.read_text", return_value=json.dumps(PASSING)), \
             redirect_stdout(io.StringIO()) as output:
            self.assertEqual(main(["coverage.json"]), 0)
        self.assertIn("value codecs", output.getvalue())
        with unittest.mock.patch("pathlib.Path.read_text", return_value="{"), \
             redirect_stdout(io.StringIO()) as output:
            self.assertEqual(main(["coverage.json"]), 1)
        self.assertIn("Coverage gate failed", output.getvalue())


if __name__ == "__main__":
    unittest.main()

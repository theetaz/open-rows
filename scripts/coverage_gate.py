# SPDX-License-Identifier: Apache-2.0
"""Gate line coverage of the core crates and the drivers' value codecs (NFR-MAINT-03).

Reads `cargo llvm-cov report --json --summary-only` from a file argument or
stdin, groups source files, prints each group and fails below the floor.
"""
import json
from pathlib import Path
import re
import sys

FLOOR_PERCENT = 80.0

# Test-only sources would inflate the numbers: they are always fully executed.
TEST_FILE = re.compile(r"(^|/)(tests?|[a-z_]+_tests)(\.rs$|/)")

# docs/17 §2.7: the gated groups and the files each one owns.
GROUPS = {
    "openrows-core": (r"crates/openrows-core/src/",),
    "openrows-sql": (r"crates/openrows-sql/src/",),
    "value codecs": (
        r"crates/openrows-driver-postgres/src/decode/",
        r"crates/openrows-driver-mysql/src/(decode\.rs|decode/|values\.rs)",
        r"crates/openrows-driver-mssql/src/decode/",
        r"crates/openrows-driver-sqlite/src/(decode\.rs|decode/)",
    ),
}


class CoverageError(Exception):
    """The report is missing, malformed or covers no gated file."""


def group_of(path):
    """The gated group a source path belongs to, or None."""
    normalised = path.replace("\\", "/")
    if TEST_FILE.search(normalised):
        return None
    for name, patterns in GROUPS.items():
        if any(re.search(pattern, normalised) for pattern in patterns):
            return name
    return None


def files_of(report):
    if not isinstance(report, dict) or not isinstance(report.get("data"), list) or not report["data"]:
        raise CoverageError("Not an llvm-cov JSON export")
    files = report["data"][0].get("files")
    if not isinstance(files, list):
        raise CoverageError("llvm-cov export has no file list")
    return files


def line_counts(entry):
    lines = (entry.get("summary") or {}).get("lines") or {}
    count, covered = lines.get("count"), lines.get("covered")
    if type(count) is not int or type(covered) is not int or not 0 <= covered <= count:
        raise CoverageError(f"Invalid line summary for {entry.get('filename')}")
    return count, covered


def group_totals(report):
    """Covered and total lines per group; every group must own at least one line."""
    totals = {name: [0, 0] for name in GROUPS}
    for entry in files_of(report):
        name = group_of(str(entry.get("filename", "")))
        if name is None:
            continue
        count, covered = line_counts(entry)
        totals[name][0] += covered
        totals[name][1] += count
    missing = [name for name, (_, count) in totals.items() if count == 0]
    if missing:
        raise CoverageError(f"No measured lines for: {', '.join(missing)}")
    return {name: dict(covered=covered, lines=count, percent=100.0 * covered / count)
            for name, (covered, count) in totals.items()}


def print_gate(totals):
    print(f'{"Group":<16}{"Lines":>9}{"Covered":>9}{"Percent":>9}  Floor {FLOOR_PERCENT:.0f} %')
    failed = 0
    for name, value in totals.items():
        passed = value["percent"] >= FLOOR_PERCENT
        failed += not passed
        print(f'{name:<16}{value["lines"]:>9}{value["covered"]:>9}{value["percent"]:>8.2f}%  '
              + ("pass" if passed else "FAIL"))
    return 1 if failed else 0


def main(arguments):
    try:
        text = Path(arguments[0]).read_text() if arguments else sys.stdin.read()
        return print_gate(group_totals(json.loads(text)))
    except (CoverageError, OSError, ValueError) as error:
        print(f"Coverage gate failed: {error}")
        return 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))

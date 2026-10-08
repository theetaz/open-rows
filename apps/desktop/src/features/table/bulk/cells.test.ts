// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import {
  dataRanges,
  inRanges,
  origin,
  parseTsv,
  pastePlan,
  selectedCells,
  selectedRows,
} from './cells';

it('drops the row-number column from grid ranges and discards empty ones', () => {
  expect(dataRanges([{ r0: 0, r1: 2, c0: 0, c1: 3 }], true)).toEqual([
    { r0: 0, r1: 2, c0: 0, c1: 2 },
  ]);
  expect(dataRanges([{ r0: 0, r1: 0, c0: 0, c1: 0 }], true)).toEqual([]);
  expect(dataRanges([{ r0: 1, r1: 1, c0: 2, c1: 2 }], false)).toEqual([
    { r0: 1, r1: 1, c0: 2, c1: 2 },
  ]);
});

it('lists distinct cells and rows across overlapping ranges, bounded by the cap', () => {
  const ranges = [
    { r0: 0, r1: 1, c0: 0, c1: 1 },
    { r0: 1, r1: 2, c0: 1, c1: 1 },
  ];
  expect(selectedCells(ranges)).toEqual([
    { row: 0, column: 0 },
    { row: 0, column: 1 },
    { row: 1, column: 0 },
    { row: 1, column: 1 },
    { row: 2, column: 1 },
  ]);
  expect(selectedCells(ranges, 2)).toHaveLength(2);
  expect(selectedRows(ranges)).toEqual([0, 1, 2]);
  expect(inRanges(ranges, { row: 2, column: 1 })).toBe(true);
  expect(inRanges(ranges, { row: 2, column: 0 })).toBe(false);
  expect(origin(ranges)).toEqual({ row: 0, column: 0 });
  expect(origin([])).toBeNull();
});

it('parses TSV with quoted tabs, line breaks, doubled quotes and CRLF', () => {
  expect(parseTsv('a\tb\n1\t2\n')).toEqual([
    ['a', 'b'],
    ['1', '2'],
  ]);
  expect(parseTsv('a\tb\r\n1\t2')).toEqual([
    ['a', 'b'],
    ['1', '2'],
  ]);
  expect(parseTsv('"x\ty"\t"say ""hi"""\n"multi\nline"\t')).toEqual([
    ['x\ty', 'say "hi"'],
    ['multi\nline', ''],
  ]);
  expect(parseTsv('')).toEqual([]);
  expect(parseTsv('single')).toEqual([['single']]);
  expect(parseTsv('NULL\t\n')).toEqual([['NULL', '']]);
});

it('lays a block from the origin and clips it to the grid', () => {
  const plan = pastePlan(
    [
      ['a', 'b', 'c'],
      ['d', 'e', 'f'],
      ['g', 'h', 'i'],
    ],
    { row: 1, column: 1 },
    { rows: 3, columns: 3 },
  );
  expect(plan).toEqual([
    { cell: { row: 1, column: 1 }, text: 'a' },
    { cell: { row: 1, column: 2 }, text: 'b' },
    { cell: { row: 2, column: 1 }, text: 'd' },
    { cell: { row: 2, column: 2 }, text: 'e' },
  ]);
});

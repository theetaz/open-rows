// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { aggregate, aggregateText } from './aggregates';

const number = { name: 'n', kind: 'number' as const };
const text = { name: 't', kind: 'text' as const };

it('sums, averages and bounds numeric selections, ignoring NULL', () => {
  const result = aggregate({
    columns: [number, number],
    rows: [
      ['1', '2.5'],
      [null, '-4'],
      ['10', '1e2'],
    ],
  });
  expect(result).toEqual({
    count: 6,
    numeric: { sum: 109.5, avg: 21.9, min: -4, max: 100 },
    distinct: null,
  });
  expect(aggregateText(result)).toBe(
    'Count: 6 · Sum: 109.5 · Avg: 21.9 · Min: -4 · Max: 100',
  );
});

it('counts distinct values of text or mixed selections', () => {
  const mixed = aggregate({
    columns: [text, number],
    rows: [
      ['a', '1'],
      ['a', null],
      ['b', '1'],
    ],
  });
  expect(mixed).toEqual({ count: 6, numeric: null, distinct: 3 });
  expect(aggregateText(mixed)).toBe('Count: 6 · Distinct: 3');
  expect(
    aggregate({ columns: [number], rows: [['12'], ['n/a']] }).numeric,
  ).toBeNull();
  expect(aggregate({ columns: [number], rows: [[null]] })).toEqual({
    count: 1,
    numeric: null,
    distinct: 0,
  });
});

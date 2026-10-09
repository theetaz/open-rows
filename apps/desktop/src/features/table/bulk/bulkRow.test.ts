// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { bulkFields, columnRanges, pageRows } from './bulkRow';
import type { RowField } from '../row';

const field = (
  name: string,
  text: string,
  state: RowField['state'] = 'normal',
): RowField => ({ name, index: 0, type: 'text', text, state });

it('shows a common value, or several, per field of the selected rows', () => {
  const fields = bulkFields([
    [field('a', '1'), field('b', 'x'), field('c', 'NULL', 'null')],
    [field('a', '1'), field('b', 'y', 'changed'), field('c', 'NULL', 'null')],
  ]);
  expect(fields).toEqual([
    { name: 'a', type: 'text', common: '1', changed: false },
    { name: 'b', type: 'text', changed: true },
    { name: 'c', type: 'text', common: null, changed: false },
  ]);
  expect(bulkFields([])).toEqual([]);
});

it('targets the selected page rows in runs on one column', () => {
  const rows = pageRows(
    [
      { r0: 0, r1: 2, c0: 0, c1: 3 },
      { r0: 5, r1: 6, c0: 1, c1: 1 },
      { r0: 9, r1: 12, c0: 0, c1: 0 },
    ],
    10,
  );
  expect(rows).toEqual([0, 1, 2, 5, 6, 9]);
  expect(columnRanges(rows, 4)).toEqual([
    { r0: 0, r1: 2, c0: 4, c1: 4 },
    { r0: 5, r1: 6, c0: 4, c1: 4 },
    { r0: 9, r1: 9, c0: 4, c1: 4 },
  ]);
});

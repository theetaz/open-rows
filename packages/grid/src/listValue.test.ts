// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { arrayItem, itemType, listValue } from './listValue';
import type { ColumnChunk } from './tiles';

const child: ColumnChunk = {
  length: 4,
  offset: 0,
  stride: 1,
  nullCount: 0,
  values: Int32Array.of(1, 2, 3, 4),
  validity: undefined,
  offsets: undefined,
};

it('quotes array items like array_out', () => {
  expect(['plain', '', 'a b', 'NULL', 'q"\\', '{x}'].map(arrayItem)).toEqual([
    'plain',
    '""',
    '"a b"',
    '"NULL"',
    '"q\\"\\\\"',
    '"{x}"',
  ]);
});

it('formats each list row from its slice of the element chunk', () => {
  const list: ColumnChunk = {
    ...child,
    length: 3,
    values: new Uint8Array(0),
    offsets: Int32Array.of(0, 3, 3, 4),
    child,
  };
  const item = (chunk: ColumnChunk, type: string, row: number) => {
    expect(type).toBe('Int32');
    const value = (chunk.values as Int32Array)[row] ?? 0;
    return value === 2 ? null : String(value);
  };
  expect(itemType('List<Int32>')).toBe('Int32');
  expect(listValue(list, 'List<Int32>', 0, item)).toBe('{1,NULL,3}');
  expect(listValue(list, 'List<Int32>', 1, item)).toBe('{}');
  expect(listValue(list, 'List<Int32>', 2, item)).toBe('{4}');
});

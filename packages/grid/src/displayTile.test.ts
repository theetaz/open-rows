// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { convertDisplayTile } from './displayTile';

it('marks NULL and empty strings like TablePlus', () => {
  const bytes = new TextEncoder().encode('ab');
  const tile = convertDisplayTile(
    {
      rows: 3,
      columns: [
        {
          name: 'name',
          type: 'Utf8',
          metadata: {},
          chunks: [
            {
              length: 3,
              offset: 0,
              stride: 1,
              nullCount: 1,
              values: bytes,
              validity: Uint8Array.of(0b011),
              offsets: Int32Array.of(0, 2, 2, 2),
            },
          ],
        },
      ],
    },
    { resultId: 'r', rowStart: 0, rowCount: 3, colStart: 0, colCount: 1 },
  );
  expect(tile.cells).toEqual([
    { text: 'ab', state: 'normal' },
    { text: 'EMPTY', state: 'empty' },
    { text: 'NULL', state: 'null' },
  ]);
});

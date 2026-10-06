// SPDX-License-Identifier: Apache-2.0
import { readFileSync } from 'node:fs';
import { expect, it } from 'vitest';
import { TileSource } from '@openrows/grid';
import { decodeTile } from '../results/decode';
it('renders S1 golden Arrow buffers with exact large integers, decimals, NULL and mixed types', async () => {
  const bytes = readFileSync(
    `${process.cwd()}/apps/desktop/src/results/fixtures/mixed.arrow`,
  );
  const { tile } = decodeTile(
    bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength),
  );
  const columns = tile.columns.map((column) => ({
    id: column.name,
    title: column.name,
    width: 120,
    pinned: false,
  }));
  const source = new TileSource('golden', tile.rows, columns, () =>
    Promise.resolve(tile),
  );
  await source.window(
    { r0: 0, r1: tile.rows - 1, c0: 0, c1: tile.columns.length - 1 },
    0,
  );
  expect(read(source, 1, 0)).toBe('658142');
  expect(read(source, 1, 1)).toBe('9007199255399135');
  expect(read(source, 1, 2)).toBe('82267.75');
  expect(read(source, 1, 3)).toBe('true');
  expect(read(source, 1, 4)).toBe('短-658142');
  expect(read(source, 1, 6)).toBe('2023-11-14T22:13:20.658Z');
  expect(read(source, 1, 7)).toBe('1234567890123522.6032');
  expect(read(source, 1, 8)).toBe('8 bytes');
  expect(read(source, 1, 9)).toBe('{"value":658142}');
  expect(read(source, 4, 1)).toBe('NULL');
  for (let row = 0; row < tile.rows; row++) {
    const original = row + 3,
      n = (original * 1_664_525 + 42) % 1_000_000;
    expect(read(source, row, 1)).toBe(
      original % 7 === 0 ? 'NULL' : String(9_007_199_254_740_993n + BigInt(n)),
    );
  }
});

function read(source: TileSource, row: number, column: number): string {
  const cell = source.cell(row, column);
  if (!cell) throw new Error('Missing golden cell');
  return cell.text;
}

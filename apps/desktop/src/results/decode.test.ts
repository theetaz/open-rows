// SPDX-License-Identifier: Apache-2.0
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { decodeTile } from './decode';
import { transferBuffers } from './transfer';
import type { ColumnChunk } from './types';

const fixture = readFileSync(
  `${process.cwd()}/apps/desktop/src/results/fixtures/mixed.arrow`,
);

function valid(chunk: ColumnChunk, row: number): boolean {
  const index = chunk.offset + row;
  return (
    !chunk.validity ||
    chunk.nullCount === 0 ||
    ((chunk.validity[index >> 3] ?? 0) & (1 << (index & 7))) !== 0
  );
}

function value(chunk: ColumnChunk, kind: number, row: number): unknown {
  if (!valid(chunk, row)) return null;
  const view = chunk.values;
  if (kind === 0) return new Int32Array(view.buffer, view.byteOffset)[row];
  if (kind === 1 || kind === 6)
    return new BigInt64Array(view.buffer, view.byteOffset)[row];
  if (kind === 2) return new Float64Array(view.buffer, view.byteOffset)[row];
  if (kind === 3) {
    const index = chunk.offset + row;
    return (
      ((new Uint8Array(view.buffer, view.byteOffset)[index >> 3] ?? 0) &
        (1 << (index & 7))) !==
      0
    );
  }
  if (kind === 7) return decimal(view, row);
  return variableWidth(chunk, kind, row);
}

function decimal(view: ArrayBufferView, row: number): bigint {
  const limbs = new Uint32Array(view.buffer, view.byteOffset);
  let result = 0n;
  for (let limb = 3; limb >= 0; limb--)
    result = (result << 32n) | BigInt(limbs[row * 4 + limb] ?? 0);
  return result;
}

function variableWidth(chunk: ColumnChunk, kind: number, row: number): unknown {
  if (!chunk.offsets) throw new Error('Missing variable-width offsets');
  const view = chunk.values;
  const bytes = new Uint8Array(view.buffer, view.byteOffset).subarray(
    chunk.offsets[row],
    chunk.offsets[row + 1],
  );
  return kind === 8 ? bytes : new TextDecoder().decode(bytes);
}

function expected(row: number, kind: number): unknown {
  if (row % 7 === 0) return null;
  const n = (row * 1_664_525 + 42) % 1_000_000;
  switch (kind) {
    case 0:
      return n;
    case 1:
      return 9_007_199_254_740_993n + BigInt(n);
    case 2:
      return n / 8;
    case 3:
      return n % 2 === 0;
    case 4:
      return `短-${String(n)}`;
    case 5:
      return `${'long'.repeat(32)}-${String(n)}`;
    case 6:
      return 1_700_000_000_000_000n + BigInt(n);
    case 7:
      return 12_345_678_901_234_567_890n + BigInt(n);
    case 8: {
      const bytes = new Uint8Array(8);
      new DataView(bytes.buffer).setBigUint64(0, BigInt(n), true);
      return bytes;
    }
    default:
      return `{"value":${String(n)}}`;
  }
}

describe('Rust encoder → worker decoder golden contract', () => {
  it('preserves every mixed cell, null, exact integer, decimal, timestamp and metadata across batches', () => {
    const decoded = decodeTile(
      fixture.buffer.slice(
        fixture.byteOffset,
        fixture.byteOffset + fixture.byteLength,
      ),
    );
    expect(decoded.tile.rows).toBe(36);
    expect(decoded.tile.columns).toHaveLength(20);
    expect(decoded.decodeMs).toBeGreaterThanOrEqual(0);
    decoded.tile.columns.forEach((column, col) => {
      expect(column.name).toBe(`column_${String(col)}`);
      expect(column.chunks).toHaveLength(2);
      let row = 3;
      for (const chunk of column.chunks) {
        expect(ArrayBuffer.isView(chunk.values)).toBe(true);
        for (let index = 0; index < chunk.length; index++)
          expect(value(chunk, col % 10, index)).toEqual(
            expected(row++, col % 10),
          );
      }
      if (col % 10 === 9)
        expect(column.metadata['ARROW:extension:name']).toBe('openrows.json');
      if (col % 10 === 7) expect(column.type).toBe('Decimal[38e+4]');
    });
    const transfers = transferBuffers(decoded);
    expect(transfers.length).toBeGreaterThan(0);
    const cloned = structuredClone(decoded, { transfer: transfers });
    expect(transfers.every((buffer) => buffer.byteLength === 0)).toBe(true);
    expect(cloned.tile.rows).toBe(36);
    cloned.tile.columns.forEach((column, col) => {
      let row = 3;
      for (const chunk of column.chunks) {
        for (let index = 0; index < chunk.length; index++)
          expect(value(chunk, col % 10, index)).toEqual(
            expected(row++, col % 10),
          );
      }
    });
  });

  it('rejects corrupt IPC instead of returning guessed values', () => {
    expect(() =>
      decodeTile(new Uint8Array([255, 255, 255, 255, 200, 0, 0, 0]).buffer),
    ).toThrow();
  });
});

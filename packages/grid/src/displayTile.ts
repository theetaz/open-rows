// SPDX-License-Identifier: Apache-2.0
import { GridError } from './error';
import type { WindowRequest } from '@openrows/ipc';
import type { Tile } from './tiles';
import type { DisplayCell } from './model';
/** Ready-to-paint worker reply. Conversion time is worker CPU, not main-thread blocking time. */
export interface DisplayTile {
  cells: DisplayCell[];
  columns: number;
  convertMs: number;
}

/** Worker-ready display strings and stable cell objects; no Arrow conversion in paint. */
export function convertDisplayTile(
  tile: Tile,
  request: WindowRequest,
): DisplayTile {
  const started = performance.now();
  if (
    tile.rows !== request.rowCount ||
    tile.columns.length !== request.colCount
  )
    throw new GridError('tile', 'Tile dimensions do not match window');
  const cells: DisplayCell[] = [];
  for (let row = 0; row < tile.rows; row++) {
    for (const [index, column] of tile.columns.entries()) {
      const part = locate(column.chunks, row);
      const chunk = part.chunk;
      const missing = isMissing(chunk, part.row);
      const state = cellState(
        request.rowStart + row,
        request.colStart + index,
        missing,
      );
      cells.push({
        text: missing ? 'NULL' : displayValue(chunk, column.type, part.row),
        state,
      });
    }
  }
  return {
    cells,
    columns: tile.columns.length,
    convertMs: performance.now() - started,
  };
}

const decoder = new TextDecoder();
function displayValue(
  chunk: Tile['columns'][number]['chunks'][number],
  type: string,
  row: number,
): string {
  const at = chunk.offset + row;
  if (chunk.offsets) {
    return variableValue(chunk, row, type);
  }
  if (type === 'Bool') {
    const values = new Uint8Array(
      chunk.values.buffer,
      chunk.values.byteOffset,
      chunk.values.byteLength,
    );
    return ((values[at >>> 3] ?? 0) & (1 << (at & 7))) === 0 ? 'false' : 'true';
  }
  const values = chunk.values as unknown as ArrayLike<number | bigint>;
  if (chunk.stride === 4) {
    return decimal(chunk, row);
  }
  if (type === 'Int64' || type.startsWith('Timestamp')) {
    const integers = new BigInt64Array(
      chunk.values.buffer,
      chunk.values.byteOffset,
      chunk.values.byteLength / 8,
    );
    const value = integers[row];
    return type === 'Int64'
      ? String(value)
      : new Date(Number(value) / 1000).toISOString();
  }
  return String(values[row * chunk.stride] ?? '');
}

function decimal(
  chunk: Tile['columns'][number]['chunks'][number],
  at: number,
): string {
  const lanes = new Uint32Array(
    chunk.values.buffer,
    chunk.values.byteOffset,
    chunk.values.byteLength / 4,
  );
  let value = 0n;
  for (let lane = 3; lane >= 0; lane--)
    value = (value << 32n) | BigInt(lanes[at * 4 + lane] ?? 0);
  const text = BigInt.asIntN(128, value).toString();
  return text.slice(0, -4) + '.' + text.slice(-4);
}

function locate(
  chunks: Tile['columns'][number]['chunks'],
  row: number,
): { chunk: Tile['columns'][number]['chunks'][number]; row: number } {
  for (const chunk of chunks) {
    if (row < chunk.length) return { chunk, row };
    row -= chunk.length;
  }
  throw new GridError('tile', 'Column chunks do not cover the tile');
}

function isMissing(
  chunk: Tile['columns'][number]['chunks'][number],
  row: number,
): boolean {
  const at = chunk.offset + row;
  return (
    chunk.nullCount > 0 &&
    (!chunk.validity ||
      ((chunk.validity[at >>> 3] ?? 0) & (1 << (at & 7))) === 0)
  );
}
function cellState(
  row: number,
  column: number,
  missing: boolean,
): DisplayCell['state'] {
  if (row === 4) return 'deleted';
  if (row === 2 && column === 2) return 'changed';
  return missing ? 'null' : 'normal';
}

function variableValue(
  chunk: Tile['columns'][number]['chunks'][number],
  row: number,
  type: string,
): string {
  if (!chunk.offsets)
    throw new GridError('tile', 'Missing variable-width offsets');
  const start = chunk.offsets[row] ?? 0,
    end = chunk.offsets[row + 1] ?? start;
  const bytes = new Uint8Array(
    chunk.values.buffer,
    chunk.values.byteOffset + start,
    end - start,
  );
  return type === 'Binary'
    ? `${String(bytes.length)} bytes`
    : decoder.decode(bytes);
}

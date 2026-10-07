// SPDX-License-Identifier: Apache-2.0
import { GridError } from './error';
import type { WindowRequest } from '@openrows/ipc';
import type { ColumnChunk, Tile } from './tiles';
import type { DisplayCell } from './model';
import { fixedValue } from './format';
/** Ready-to-paint worker reply. Conversion time is worker CPU, not main-thread blocking time. */
export interface DisplayTile {
  cells: DisplayCell[];
  columns: number;
  convertMs: number;
}

/** The synthetic benchmark result shows sample edit states so every paint layer is measured. */
const SPIKE_RESULT = 'spike';

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
  const spike = request.resultId === SPIKE_RESULT;
  const cells: DisplayCell[] = [];
  for (let row = 0; row < tile.rows; row++) {
    for (const [index, column] of tile.columns.entries()) {
      const part = locate(column.chunks, row);
      const missing = isMissing(part.chunk, part.row);
      cells.push({
        text: missing
          ? 'NULL'
          : displayValue(part.chunk, column.type, part.row),
        state: spike
          ? spikeState(
              request.rowStart + row,
              request.colStart + index,
              missing,
            )
          : missing
            ? 'null'
            : 'normal',
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
function displayValue(chunk: ColumnChunk, type: string, row: number): string {
  if (chunk.offsets) return variableValue(chunk, row, type);
  if (type === 'Bool') {
    const at = chunk.offset + row;
    const values = new Uint8Array(
      chunk.values.buffer,
      chunk.values.byteOffset,
      chunk.values.byteLength,
    );
    return ((values[at >>> 3] ?? 0) & (1 << (at & 7))) === 0 ? 'false' : 'true';
  }
  return fixedValue(chunk, type, row);
}

function locate(
  chunks: Tile['columns'][number]['chunks'],
  row: number,
): { chunk: ColumnChunk; row: number } {
  for (const chunk of chunks) {
    if (row < chunk.length) return { chunk, row };
    row -= chunk.length;
  }
  throw new GridError('tile', 'Column chunks do not cover the tile');
}

function isMissing(chunk: ColumnChunk, row: number): boolean {
  const at = chunk.offset + row;
  return (
    chunk.nullCount > 0 &&
    (!chunk.validity ||
      ((chunk.validity[at >>> 3] ?? 0) & (1 << (at & 7))) === 0)
  );
}
function spikeState(
  row: number,
  column: number,
  missing: boolean,
): DisplayCell['state'] {
  if (row === 4) return 'deleted';
  if (row === 2 && column === 2) return 'changed';
  return missing ? 'null' : 'normal';
}

function variableValue(chunk: ColumnChunk, row: number, type: string): string {
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

// SPDX-License-Identifier: Apache-2.0
import { GridError } from './error';
import type { WindowRequest } from '@openrows/ipc';
import type { ColumnChunk, Tile } from './tiles';
import type { DisplayCell } from './model';
import { fixedValue } from './format';
import { documentValue } from './documentValue';
import { listValue } from './listValue';
/** Ready-to-paint worker reply. Conversion time is worker CPU, not main-thread blocking time. */
export interface DisplayTile {
  cells: DisplayCell[];
  columns: number;
  convertMs: number;
}

/** Field metadata key of the source's fractional-second precision (docs/04 §5). */
const FRACTION_DIGITS = 'openrows.fraction_digits';

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
  const digits = tile.columns.map(fractionDigits);
  for (let row = 0; row < tile.rows; row++) {
    for (const [index, column] of tile.columns.entries()) {
      const part = locate(column.chunks, row);
      const missing = isMissing(part.chunk, part.row);
      const value = missing
        ? ''
        : displayValue(part.chunk, column.type, part.row, digits[index]);
      cells.push({
        text: missing ? 'NULL' : value || 'EMPTY',
        state: spike
          ? spikeState(
              request.rowStart + row,
              request.colStart + index,
              missing,
            )
          : markerState(missing, value),
      });
    }
  }
  return {
    cells,
    columns: tile.columns.length,
    convertMs: performance.now() - started,
  };
}

/** NULL and empty strings paint as dimmed markers, like TablePlus. */
function markerState(missing: boolean, value: string): DisplayCell['state'] {
  if (missing) return 'null';
  return value ? 'normal' : 'empty';
}

/** The source's fixed fractional-second digits (`openrows.fraction_digits`), if any. */
function fractionDigits(column: Tile['columns'][number]): number | undefined {
  const digits = Number(column.metadata[FRACTION_DIGITS] ?? Number.NaN);
  return Number.isInteger(digits) && digits >= 0 && digits <= 9
    ? digits
    : undefined;
}

const decoder = new TextDecoder();
function displayValue(
  chunk: ColumnChunk,
  type: string,
  row: number,
  digits: number | undefined,
): string {
  if (chunk.fields || (chunk.document && type.startsWith('List<')))
    return documentValue(chunk, type, row, scalarValue);
  if (type.startsWith('List<')) return listValue(chunk, type, row, itemText);
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
  return fixedValue(chunk, type, row, digits);
}

/** One array element as `PostgreSQL` prints it (binary as `\x` hex). */
function itemText(
  chunk: ColumnChunk,
  type: string,
  row: number,
): string | null {
  if (isMissing(chunk, row)) return null;
  if (type !== 'Binary') return displayValue(chunk, type, row, undefined);
  const start = chunk.offsets?.[row] ?? 0;
  const end = chunk.offsets?.[row + 1] ?? start;
  const bytes = new Uint8Array(
    chunk.values.buffer,
    chunk.values.byteOffset + start,
    end - start,
  );
  return `\\x${Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('')}`;
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

function scalarValue(chunk: ColumnChunk, type: string, row: number): string {
  return displayValue({ ...chunk, document: false }, type, row, undefined);
}

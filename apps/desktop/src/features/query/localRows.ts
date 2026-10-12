// SPDX-License-Identifier: Apache-2.0
import type { DisplayCell } from '@openrows/grid';
import type { WindowRequest } from '@openrows/ipc';
import type { DisplayTile } from '@openrows/grid/display-tile';
import type { ResultMeta } from './run';
import { cellMatches } from './columnFilters';
import type { ColumnFilter } from './columnFilters';

/**
 * Most cells (rows × columns) a result may have for client-side sort and
 * filter; their display text is held in memory while either is active.
 */
export const LOCAL_CELL_LIMIT = 1_000_000;
/** Rows per window while loading. */
const CHUNK_ROWS = 4096;

type Load = (request: WindowRequest) => Promise<DisplayTile>;

/** Why sort and filter are unavailable for a result, or `null` when they work. */
export function localLimit(result: ResultMeta): string | null {
  if (!result.done) return 'Sort and filter work once every row has arrived.';
  if (result.rows * result.columns.length > LOCAL_CELL_LIMIT)
    return `Sort and filter work on results up to ${LOCAL_CELL_LIMIT.toLocaleString('en-US')} cells; add ORDER BY or WHERE to the query.`;
  return null;
}

/** Every display cell of a complete result, read once through result windows. */
export class LocalRows {
  private constructor(
    readonly rows: number,
    readonly columns: number,
    private readonly chunks: readonly (readonly DisplayCell[])[],
  ) {}

  /** Read all rows in chunks; the caller checks `localLimit` first. */
  static async load(result: ResultMeta, load: Load): Promise<LocalRows> {
    const columns = result.columns.length;
    const starts: number[] = [];
    for (let start = 0; start < result.rows; start += CHUNK_ROWS)
      starts.push(start);
    const tiles = await Promise.all(
      starts.map((rowStart) =>
        load({
          resultId: result.id,
          rowStart,
          rowCount: Math.min(CHUNK_ROWS, result.rows - rowStart),
          colStart: 0,
          colCount: columns,
        }),
      ),
    );
    return new LocalRows(
      result.rows,
      columns,
      tiles.map((tile) => tile.cells),
    );
  }

  cell(row: number, column: number): DisplayCell | undefined {
    const chunk = this.chunks[Math.floor(row / CHUNK_ROWS)];
    return chunk?.[(row % CHUNK_ROWS) * this.columns + column];
  }
}

/** A client-side sort by one result column. */
export type RowOrder = { column: number; descending: boolean };

/** The any-column search text and the header quick filters of a result. */
type RowFilter = { needle: string; columns: readonly ColumnFilter[] };

/** Whether any column's display text contains `search` (lower case). */
function rowContains(local: LocalRows, row: number, search: string): boolean {
  for (let column = 0; column < local.columns; column++) {
    const cell = local.cell(row, column);
    if (cell?.state !== 'null' && cell?.text.toLowerCase().includes(search))
      return true;
  }
  return false;
}

/** Rows whose display text contains `needle` (case-insensitive) in any column. */
export function filterRows(local: LocalRows, needle: string): number[] {
  return matchingRows(local, { needle, columns: [] });
}

/** Rows matching the search text (when any) and every column filter. */
function matchingRows(local: LocalRows, filter: RowFilter): number[] {
  const search = filter.needle.trim().toLowerCase();
  const rows: number[] = [];
  for (let row = 0; row < local.rows; row++) {
    if (search && !rowContains(local, row, search)) continue;
    const passes = filter.columns.every((item) =>
      cellMatches(local.cell(row, item.column), item),
    );
    if (passes) rows.push(row);
  }
  return rows;
}

/** Whether a filter narrows the rows at all. */
function filtering(filter: RowFilter): boolean {
  return filter.needle.trim() !== '' || filter.columns.length > 0;
}

/** Sort key of a cell: `null` for NULL, a number for numeric columns. */
function key(
  cell: DisplayCell | undefined,
  numeric: boolean,
): string | number | null {
  if (!cell || cell.state === 'null') return null;
  if (!numeric) return cell.state === 'empty' ? '' : cell.text;
  const value = Number(cell.text);
  return Number.isNaN(value) ? cell.text : value;
}

function compare(a: string | number | null, b: string | number | null): number {
  if (a === b) return 0;
  if (a === null) return 1;
  if (b === null) return -1;
  if (typeof a !== typeof b) return typeof a === 'number' ? -1 : 1;
  return a < b ? -1 : 1;
}

/** Stable sort of `rows`; NULLs sort last in both directions. */
export function sortRows(
  local: LocalRows,
  rows: readonly number[],
  order: RowOrder,
  numeric: boolean,
): number[] {
  const keys: (string | number | null)[] = [];
  for (const row of rows)
    keys[row] = key(local.cell(row, order.column), numeric);
  const sign = order.descending ? -1 : 1;
  return [...rows].sort((a, b) => {
    const left = keys[a] ?? null,
      right = keys[b] ?? null;
    if (left === null || right === null) return compare(left, right);
    return sign * compare(left, right);
  });
}

/**
 * The shown result rows for a filter and sort, or `null` for every row in
 * result order.
 */
export function viewRows(
  local: LocalRows,
  filter: RowFilter,
  order: RowOrder | null,
  numeric: (column: number) => boolean,
): Int32Array | null {
  const narrowed = filtering(filter);
  if (!narrowed && !order) return null;
  let rows = narrowed
    ? matchingRows(local, filter)
    : Array.from({ length: local.rows }, (_, row) => row);
  if (order) rows = sortRows(local, rows, order, numeric(order.column));
  return Int32Array.from(rows);
}

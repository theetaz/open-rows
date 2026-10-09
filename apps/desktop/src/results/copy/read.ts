// SPDX-License-Identifier: Apache-2.0
import type { CellRange, DisplayCell } from '@openrows/grid';
import type { WindowRequest } from '@openrows/ipc';
import type { DisplayTile } from '@openrows/grid/display-tile';
import type { ResultMeta } from '../../features/query/run';
import { columnAlign } from '../align';
import { valueKind } from './sql';
import type { CopyTable } from './formats';

/** Most rows one copy reads; larger selections ask for Export instead. */
export const COPY_ROW_LIMIT = 100_000;
/** Rows one window may span, so scattered rows never fetch a huge gap. */
const SPAN_ROWS = 2048;

/** A selection that cannot be copied, e.g. too many rows. */
export class CopyError extends Error {
  override readonly name = 'CopyError';
}

/** Which cells to copy, in view coordinates, and how they map to the result. */
export type CopySource = {
  result: ResultMeta;
  /** Selected ranges over data columns (no row-number gutter). */
  ranges: readonly CellRange[];
  /** View row → result row (client sort/filter); identity when absent. */
  rowOf?: (row: number) => number;
  /** View column → result column (reordered columns); identity when absent. */
  columnOf?: (column: number) => number;
};

type Load = (request: WindowRequest) => Promise<DisplayTile>;

/** Distinct ascending view rows of the ranges, refusing more than the limit. */
function viewRows(ranges: readonly CellRange[]): number[] {
  const bound = ranges.reduce((sum, range) => sum + range.r1 - range.r0 + 1, 0);
  if (bound > COPY_ROW_LIMIT * 4) throw new CopyError(tooMany());
  const rows = new Set<number>();
  for (const range of ranges)
    for (let row = range.r0; row <= range.r1; row++) rows.add(row);
  if (rows.size > COPY_ROW_LIMIT) throw new CopyError(tooMany());
  return [...rows].sort((a, b) => a - b);
}

function tooMany(): string {
  return `Copy is limited to ${COPY_ROW_LIMIT.toLocaleString('en-US')} rows; use Export… for more.`;
}

/** Distinct ascending view columns of the ranges. */
function viewColumns(ranges: readonly CellRange[]): number[] {
  const columns = new Set<number>();
  for (const range of ranges)
    for (let column = range.c0; column <= range.c1; column++)
      columns.add(column);
  return [...columns].sort((a, b) => a - b);
}

/** Group ascending result rows into windows of at most `SPAN_ROWS` rows. */
function spans(rows: readonly number[]): number[][] {
  const groups: number[][] = [];
  for (const row of rows) {
    const last = groups.at(-1);
    const start = last?.[0];
    if (last && start !== undefined && row - start < SPAN_ROWS) last.push(row);
    else groups.push([row]);
  }
  return groups;
}

function value(cell: DisplayCell | undefined): string | null {
  if (!cell || cell.state === 'null') return null;
  return cell.state === 'empty' ? '' : cell.text;
}

/** Read the selected cells' display text from the result store. */
export async function readSelection(
  source: CopySource,
  load: Load,
): Promise<CopyTable> {
  const { result } = source;
  const rowOf = source.rowOf ?? ((row: number) => row);
  const columnOf = source.columnOf ?? ((column: number) => column);
  const rows = viewRows(source.ranges).map(rowOf);
  const columns = viewColumns(source.ranges).map(columnOf);
  if (!rows.length || !columns.length) return { columns: [], rows: [] };
  const first = Math.min(...columns),
    width = Math.max(...columns) - first + 1;
  const cells = new Map<number, (string | null)[]>();
  const sorted = [...new Set(rows)].sort((a, b) => a - b);
  await Promise.all(
    spans(sorted).map(async (group) => {
      const start = group[0] ?? 0;
      const tile = await load({
        resultId: result.id,
        rowStart: start,
        rowCount: (group.at(-1) ?? start) - start + 1,
        colStart: first,
        colCount: width,
      });
      for (const row of group)
        cells.set(
          row,
          columns.map((column) =>
            value(tile.cells[(row - start) * width + column - first]),
          ),
        );
    }),
  );
  return {
    columns: columns.map((index) => {
      const column = result.columns[index];
      const type = column?.sourceType ?? null;
      return {
        name: column?.name ?? '',
        kind: valueKind(type, columnAlign(type) === 'end'),
      };
    }),
    rows: rows.map((row) => cells.get(row) ?? columns.map(() => null)),
  };
}

// SPDX-License-Identifier: Apache-2.0
import type { CellRange } from '@openrows/grid';
import type { ResultMeta } from '../query/run';
import { inRanges, selectedRows } from './bulk/cells';
import type { Cell } from './bulk/cells';
import { readRows, shownFields } from './row';
import type { RowField } from './row';

/** The table grid's selection in result (data) coordinates. */
export type GridSelection = {
  result: ResultMeta;
  /** The focused cell. */
  cell: Cell;
  ranges: readonly CellRange[];
};

/** One TSV field: quoted when it holds a tab, line break or quote. */
function tsv(value: string): string {
  return /[\t\n\r"]/.test(value) ? `"${value.replaceAll('"', '""')}"` : value;
}

/** A row as TSV: the shown fields, `EMPTY` strings as nothing. */
export function rowText(fields: readonly RowField[]): string {
  return fields
    .map((item) => tsv(item.state === 'empty' ? '' : item.text))
    .join('\t');
}

/**
 * The selection as TSV: the selected cells of each selected page row in
 * column order, or the focused row when nothing is range-selected. Staged
 * inserts have no stored row and are left out.
 */
export async function selectionText(selection: GridSelection): Promise<string> {
  const { result, cell } = selection;
  const ranges: readonly CellRange[] = selection.ranges.length
    ? selection.ranges
    : [
        {
          r0: cell.row,
          r1: cell.row,
          c0: 0,
          c1: result.columns.length - 1,
        },
      ];
  const rows = selectedRows(ranges).filter((row) => row < result.rows);
  const fields = await readRows(result, rows);
  return rows
    .map((row) =>
      rowText(
        shownFields(fields.get(row) ?? []).filter((field) =>
          inRanges(ranges, { row, column: field.index }),
        ),
      ),
    )
    .join('\n');
}

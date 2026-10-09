// SPDX-License-Identifier: Apache-2.0
import type { DisplayCell } from '@openrows/grid';
import type { TableColumnDto } from '@openrows/ipc';
import { displayWindow } from '../../results/displayWindow';
import type { ResultMeta } from '../query/run';
import type { CellValue } from './cellValue';

/** One field of the selected row, as the inspector lists it. */
export type RowField = {
  name: string;
  /** Column index within the result set (addresses the exact value). */
  index: number;
  /** Database type name, e.g. `int4`. */
  type: string | null;
  text: string;
  /** `changed` while staged; `deleted` when the row is staged for deletion. */
  state: DisplayCell['state'];
  /** Exact value (staged, or the loaded original); absent while only display text is known. */
  value?: CellValue;
};

/** Inspector editing of the selected row; every change only stages. */
export type RowEditor = {
  columns: readonly TableColumnDto[];
  /** Generated columns, which never accept values. */
  protected: readonly string[];
  /** The exact original snapshot is loaded, so fields can be edited. */
  ready: boolean;
  /** Load the exact original values (display text may be rounded or summarised). */
  load: () => void;
  stage: (name: string, value: CellValue) => void;
  /** Pick a file natively and stage its contents. */
  addFile: (name: string) => void;
};

/** Where a row's exact values live: its result set and row index in that result. */
export type RowSource = { resultId: string; row: number };

/** The selected table row: one row's display values for the Details panel. */
export type SelectedRow = {
  table: string;
  /** Row number within the table view, 1-based across pages. */
  number: number;
  fields: readonly RowField[];
  /** The stored result the row was read from; enables the full-value inspector. */
  source?: RowSource;
  /** Present when the table is editable (writable connection, stable key). */
  editor?: RowEditor;
  error?: string;
};

/** Publish the selected row; `reveal` brings the Details panel forward (default). */
export type ShowRow = (row: SelectedRow, reveal?: boolean) => void;

/** Read one row of a page from the result store for the Details panel. */
export async function readRow(
  result: ResultMeta,
  row: number,
): Promise<RowField[]> {
  const rows = await readRows(result, [row]);
  return rows.get(row) ?? [];
}

/** Read several page rows in one window (their span), keyed by row index. */
export async function readRows(
  result: ResultMeta,
  rows: readonly number[],
): Promise<Map<number, RowField[]>> {
  const fields = new Map<number, RowField[]>();
  if (!rows.length) return fields;
  const first = Math.min(...rows);
  const columns = result.columns.length;
  const tile = await displayWindow({
    resultId: result.id,
    rowStart: first,
    rowCount: Math.max(...rows) - first + 1,
    colStart: 0,
    colCount: columns,
  });
  for (const row of rows)
    fields.set(
      row,
      result.columns.map((column, index) => {
        const cell = tile.cells[(row - first) * columns + index];
        return {
          name: column.name,
          index,
          type: column.sourceType,
          text: cell?.text ?? '',
          state: cell?.state ?? 'normal',
        };
      }),
    );
  return fields;
}

/** Fields whose name contains `search` (case-insensitive). */
export function matchingFields(
  fields: readonly RowField[],
  search: string,
): readonly RowField[] {
  const needle = search.trim().toLowerCase();
  if (!needle) return fields;
  return fields.filter((field) => field.name.toLowerCase().includes(needle));
}

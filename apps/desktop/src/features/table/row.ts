// SPDX-License-Identifier: Apache-2.0
import type { DisplayCell } from '@openrows/grid';
import { displayWindow } from '../../results/displayWindow';
import type { ResultMeta } from '../query/run';

/** One field of the selected row, as the inspector lists it. */
export type RowField = {
  name: string;
  /** Database type name, e.g. `int4`. */
  type: string | null;
  text: string;
  state: DisplayCell['state'];
};

/** The selected table row: one row's display values for the Details panel. */
export type SelectedRow = {
  table: string;
  /** Row number within the table view, 1-based across pages. */
  number: number;
  fields: readonly RowField[];
};

/** Read one row of a page from the result store for the Details panel. */
export async function readRow(
  result: ResultMeta,
  row: number,
): Promise<RowField[]> {
  const tile = await displayWindow({
    resultId: result.id,
    rowStart: row,
    rowCount: 1,
    colStart: 0,
    colCount: result.columns.length,
  });
  return result.columns.map((column, index) => {
    const cell = tile.cells[index];
    return {
      name: column.name,
      type: column.sourceType,
      text: cell?.text ?? '',
      state: cell?.state ?? 'normal',
    };
  });
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

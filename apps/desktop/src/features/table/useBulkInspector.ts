// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef } from 'react';
import { canEdit } from './EditToolbar';
import { readRows, shownFields } from './row';
import type { SelectedRow, ShowRow } from './row';
import {
  BULK_READ_LIMIT,
  bulkFields,
  columnRanges,
  pageRows,
} from './bulk/bulkRow';
import type { BulkRows } from './bulk/bulkRow';
import type { GridSelection } from './gridSelection';
import type { RowEditing } from './useRowEditing';

type Args = {
  table: string;
  editing: RowEditing;
  ready: boolean;
  onRow: ShowRow;
};

/** Selected page rows when several are selected, else `null`. */
export function bulkRowsOf(selection: GridSelection | null): number[] | null {
  if (!selection) return null;
  const rows = pageRows(selection.ranges, selection.result.rows);
  return rows.length > 1 ? rows : null;
}

/**
 * While several rows are selected, show them in the inspector as one bulk
 * row (FR-GRID-14): each field's common value or "multiple values", and a
 * value typed there is staged in every selected row through the change set
 * (reviewed before commit like any edit). Republishes as edits change.
 */
export function useBulkInspector(
  args: Args,
  selection: GridSelection | null,
): void {
  const rows = bulkRowsOf(selection);
  const latest = useRef({ args, selection, rows });
  useEffect(() => {
    latest.current = { args, selection, rows };
  });
  const key = rows ? JSON.stringify([selection?.result.id, rows]) : null;
  const changes = args.editing.history.current;
  const writable = canEdit(args.editing, args.ready);
  useEffect(() => {
    const current = latest.current;
    if (!key || !current.selection || !current.rows) return;
    let live = true;
    const publish = (row: SelectedRow): void => {
      if (live) latest.current.args.onRow(row);
    };
    readBulk(current.selection, current.rows, writable, latest).then(
      (bulk) => {
        publish({ table: current.args.table, number: 0, fields: [], bulk });
      },
      (error: unknown) => {
        publish({
          table: current.args.table,
          number: 0,
          fields: [],
          error: error instanceof Error ? error.message : String(error),
        });
      },
    );
    return () => {
      live = false;
    };
  }, [key, changes, writable]);
}

/** Read the selected rows' fields and compare them. */
async function readBulk(
  { result }: GridSelection,
  rows: readonly number[],
  writable: boolean,
  latest: React.RefObject<{ args: Args }>,
): Promise<BulkRows> {
  const read = rows.slice(0, BULK_READ_LIMIT);
  const fields = await readRows(result, read);
  const shown = read.map((row) => shownFields(fields.get(row) ?? []));
  const bulk: BulkRows = { rows: rows.length, fields: bulkFields(shown) };
  if (writable)
    bulk.stage = (name, value) => {
      const column = result.columns.findIndex((item) => item.name === name);
      if (column >= 0)
        latest.current.args.editing.bulk.set(
          result,
          columnRanges(rows, column),
          value,
        );
    };
  return bulk;
}

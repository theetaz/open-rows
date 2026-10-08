// SPDX-License-Identifier: Apache-2.0
import { useEffect, useEffectEvent } from 'react';
import type { GridHandle } from '@openrows/grid';
import type { ResultMeta } from '../../query/run';
import type { EditedSource } from '../EditedSource';
import { stagedInserts } from '../EditedSource';
import type { InlineRequest, RowEditing } from '../useRowEditing';
import { columnEdit, gridKinds } from './cellKind';
import type { InlineEditor } from './useInlineEditor';

type Args = {
  result: ResultMeta;
  editing: RowEditing;
  rowNumbers: boolean;
  handle: React.RefObject<GridHandle | undefined>;
  source: React.RefObject<EditedSource | undefined>;
  editor: InlineEditor;
  /** Increments whenever a canvas finishes mounting. */
  mounted: number;
};

/** Apply column kinds once edit info arrives, and serve "+ Row" / "Edit value…" requests. */
export function useGridEditing(args: Args): void {
  const { editing, result, mounted } = args;
  const info = editing.info;
  useEffect(() => {
    if (
      mounted &&
      args.source.current?.describe(gridKinds(result.columns, info))
    )
      args.handle.current?.refresh?.();
  }, [args.source, args.handle, result, info, mounted]);
  const serve = useEffectEvent((request: InlineRequest) => {
    const grid = args.handle.current;
    if (!grid) return;
    editing.clearInline();
    const cell = requestCell(request, editing, result);
    if (!cell) return;
    grid.select?.(cell.row, cell.column + (args.rowNumbers ? 1 : 0));
    args.editor.start(cell);
  });
  useEffect(() => {
    if (editing.inline && mounted) serve(editing.inline);
  }, [editing.inline, mounted]);
}

/** A new row starts at its first editable column without a default, like TablePlus. */
function requestCell(
  request: InlineRequest,
  editing: RowEditing,
  result: ResultMeta,
): { row: number; column: number } | null {
  if (!('insert' in request)) return request;
  const index = stagedInserts(editing.history.current).findIndex(
    (edit) => edit.id === request.insert,
  );
  if (index < 0) return null;
  const editable = result.columns.flatMap((column, at) =>
    columnEdit(editing.info, column.name) ? [at] : [],
  );
  const plain = editable.find(
    (at) =>
      !editing.info?.columns.find(
        (column) => column.name === result.columns[at]?.name,
      )?.default,
  );
  const column = plain ?? editable[0];
  return column === undefined ? null : { row: result.rows + index, column };
}

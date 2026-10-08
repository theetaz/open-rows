// SPDX-License-Identifier: Apache-2.0
import type { EditInfoDto } from '@openrows/ipc';
import type { ResultMeta } from '../query/run';
import { cellDraft } from './cellEdits';
import type { EditHistory, StagedEdit } from './editHistory';
import type { TableTargetRef } from './model';
import type { EditIo } from './useEditIo';
import { fromOriginal } from './cellValue';
import type { CellValue } from './cellValue';

export type QuickEditAction = 'delete' | 'duplicate';
/** Context actions share the exact-snapshot staging path; no database write occurs. */
export function quickEdit(args: {
  io: EditIo;
  history: EditHistory;
  target: TableTargetRef;
  offset: number;
  result: ResultMeta;
  cell: { row: number; column: number };
  action: QuickEditAction;
  stage: (edit: StagedEdit) => void;
}): void {
  const { io, history, cell, action } = args;
  const info = io.info;
  if (
    !info ||
    info.readOnly ||
    !info.key.length ||
    history.pending ||
    history.uncertain
  )
    return;
  void io.run(async () => {
    const draft = await cellDraft({ ...args, info, row: cell.row });
    if (!io.alive.current) return;
    if (action === 'delete')
      args.stage({ ...draft, kind: 'delete', values: {} });
    else args.stage(duplicateRow(draft, info));
  });
}

/** Copy writable values while leaving the new row's key/generated fields to DEFAULT. */
export function duplicateRow(draft: StagedEdit, info: EditInfoDto): StagedEdit {
  return {
    id: crypto.randomUUID(),
    number: 0,
    kind: 'insert',
    original: {},
    values: Object.fromEntries(
      Object.entries({
        ...originalValues(draft.original),
        ...draft.values,
      }).filter(
        ([name]) => !info.key.includes(name) && !info.protected.includes(name),
      ),
    ),
  };
}

function originalValues(
  original: StagedEdit['original'],
): Record<string, CellValue> {
  return Object.fromEntries(
    Object.entries(original).map(([name, value]) => [
      name,
      fromOriginal(value),
    ]),
  );
}

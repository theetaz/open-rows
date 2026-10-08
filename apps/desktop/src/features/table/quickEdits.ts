// SPDX-License-Identifier: Apache-2.0
import type { EditInfoDto } from '@openrows/ipc';
import type { ResultMeta } from '../query/run';
import { editDraft } from './editingIo';
import type { EditHistory, StagedEdit } from './editHistory';
import type { TableTargetRef } from './model';
import type { EditIo } from './useEditIo';
import { fromOriginal, NULL_VALUE, textValue } from './cellValue';
import type { CellValue } from './cellValue';
import { fieldValueError } from './fieldActions';

export type QuickEditAction = 'delete' | 'duplicate' | 'null' | 'empty';
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
  const { io, history, result, cell, action } = args;
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
    const draft = await editDraft({ ...args, info, row: cell.row });
    if (!io.alive.current) return;
    if (action === 'delete')
      args.stage({ ...draft, kind: 'delete', values: {} });
    else if (action === 'duplicate') args.stage(duplicateRow(draft, info));
    else {
      const name = result.columns[cell.column]?.name ?? '';
      const value = action === 'null' ? NULL_VALUE : textValue('');
      const error = fieldValueError(
        info.columns.find((item) => item.name === name),
        info.protected.includes(name),
        value,
      );
      if (error) throw new Error(error);
      args.stage({ ...draft, values: { ...draft.values, [name]: value } });
    }
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

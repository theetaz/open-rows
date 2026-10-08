// SPDX-License-Identifier: Apache-2.0
import { pickFileValue } from '@openrows/ipc';
import type { FileEncodingDto } from '@openrows/ipc';
import type { ResultMeta } from '../query/run';
import type { CellValue } from './cellValue';
import { editDraft } from './editingIo';
import type { EditHistory, StagedEdit } from './editHistory';
import { fieldValueError } from './fieldActions';
import type { TableTargetRef } from './model';
import type { EditIo } from './useEditIo';

/** What single-field staging needs from the table's editing state. */
export type FieldContext = {
  io: EditIo;
  history: EditHistory;
  target: TableTargetRef;
  offset: number;
  change: (edit: StagedEdit) => void;
};

function editable({ io, history }: FieldContext): boolean {
  const info = io.info;
  return (
    io.alive.current &&
    !!info &&
    !info.readOnly &&
    !!info.key.length &&
    !history.pending &&
    !history.uncertain
  );
}

/** Read the row's exact server snapshot (or its staged change) without staging anything. */
export function prepareRow(
  context: FieldContext,
  at: { result: ResultMeta; row: number },
  ready: (draft: StagedEdit) => void,
): void {
  const info = context.io.info;
  if (!info || !editable(context)) return;
  void context.io.run(async () => {
    const draft = await editDraft({ ...context, ...at, info });
    if (context.io.alive.current) ready(draft);
  });
}

/** Stage one field of a prepared row; catalog rules are re-checked here, not only in menus. */
export function stageField(
  context: FieldContext,
  draft: StagedEdit,
  field: { name: string; value: CellValue },
): void {
  const info = context.io.info;
  if (!info || !editable(context)) return;
  const error = fieldValueError(
    info.columns.find((column) => column.name === field.name),
    info.protected.includes(field.name) || draft.kind === 'delete',
    field.value,
  );
  if (error) {
    void context.io.run(() => Promise.reject(new Error(error)));
    return;
  }
  const staged = context.history.current.find((item) => item.id === draft.id);
  context.change({
    ...(staged ?? draft),
    kind: staged?.kind ?? 'update',
    values: { [field.name]: field.value },
  });
}

/** Let Rust pick and read a file; `use` receives its text (cancelled: nothing). */
export function pickFile(
  context: FieldContext,
  encoding: FileEncodingDto,
  use: (text: string) => void,
): void {
  if (!editable(context)) return;
  void context.io.run(async () => {
    const text = await pickFileValue(encoding);
    if (text !== null && context.io.alive.current) use(text);
  });
}

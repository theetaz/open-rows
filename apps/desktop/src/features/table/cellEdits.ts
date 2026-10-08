// SPDX-License-Identifier: Apache-2.0
import type { EditInfoDto } from '@openrows/ipc';
import type { ResultMeta } from '../query/run';
import { editDraft } from './editingIo';
import { stagedInserts } from './EditedSource';
import type { EditHistory, StagedEdit } from './editHistory';
import type { TableTargetRef } from './model';
import type { CellValue } from './cellValue';

/** Rows and staged changes can be edited: writable, keyed, and not mid-commit. */
export function writableResult(
  info: EditInfoDto | null,
  history: EditHistory,
  result: ResultMeta,
): info is EditInfoDto {
  return (
    !!info &&
    !info.readOnly &&
    !!info.key.length &&
    !history.pending &&
    !history.uncertain &&
    info.key.every((name) =>
      result.columns.some((column) => column.name === name),
    )
  );
}

/** The exact row a cell edit merges into: a server snapshot, or a staged insert after the page. */
export function cellDraft(args: {
  info: EditInfoDto;
  result: ResultMeta;
  row: number;
  offset: number;
  target: TableTargetRef;
  history: EditHistory;
}): Promise<StagedEdit> {
  const { result, row, history } = args;
  if (row < result.rows) return editDraft(args);
  const insert = stagedInserts(history.current)[row - result.rows];
  return insert
    ? Promise.resolve(insert)
    : Promise.reject(new Error('That new row is no longer staged.'));
}

/** Stage one field only, so a draft read before other edits never reverts them. */
export function fieldEdit(
  draft: StagedEdit,
  name: string,
  value: CellValue,
): StagedEdit {
  return { ...draft, values: { [name]: value } };
}

/** A new staged row; unset columns insert DEFAULT. */
export function newInsert(): StagedEdit {
  return {
    id: crypto.randomUUID(),
    number: 0,
    kind: 'insert',
    original: {},
    values: {},
  };
}

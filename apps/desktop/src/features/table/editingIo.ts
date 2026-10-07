// SPDX-License-Identifier: Apache-2.0
import { tableEditRow } from '@openrows/ipc';
import type { EditInfoDto } from '@openrows/ipc';
import type { ResultMeta } from '../query/run';
import { readRow } from './row';
import type { TableTargetRef } from './model';
import type { EditHistory, StagedEdit } from './editHistory';

/** Resolve the displayed stable key, then read a complete exact server snapshot. */
export async function editDraft(args: {
  info: EditInfoDto;
  result: ResultMeta;
  row: number;
  offset: number;
  target: TableTargetRef;
  history: EditHistory;
}): Promise<StagedEdit> {
  const fields = await readRow(args.result, args.row);
  const keys: Record<string, string> = {};
  for (const name of args.info.key) {
    const field = fields.find((item) => item.name === name);
    if (!field || field.state === 'null')
      throw new Error('Show all unique-key columns before editing this row.');
    keys[name] = field.state === 'empty' ? '' : field.text;
  }
  const id = JSON.stringify(keys);
  const existing = args.history.current.find((item) => item.id === id);
  const original =
    existing?.original ?? (await tableEditRow(args.target, keys));
  return existing
    ? { ...existing, kind: 'update' }
    : {
        id,
        number: args.offset + args.row + 1,
        kind: 'update',
        original,
        values: {},
      };
}

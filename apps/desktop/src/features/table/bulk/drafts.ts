// SPDX-License-Identifier: Apache-2.0
import { tableEditRows } from '@openrows/ipc';
import type { EditInfoDto } from '@openrows/ipc';
import type { ResultMeta } from '../../query/run';
import { stagedInserts } from '../EditedSource';
import type { EditHistory, StagedEdit } from '../editHistory';
import type { TableTargetRef } from '../model';
import { readRows } from '../row';
import type { RowField } from '../row';

type DraftArgs = {
  info: EditInfoDto;
  result: ResultMeta;
  offset: number;
  target: TableTargetRef;
  history: EditHistory;
};

/** The stable key values of a displayed row, or why it has none. */
function keysOf(
  fields: readonly RowField[],
  key: readonly string[],
): Record<string, string> {
  const keys: Record<string, string> = {};
  for (const name of key) {
    const field = fields.find((item) => item.name === name);
    if (!field || field.state === 'null')
      throw new Error('Show all unique-key columns before editing these rows.');
    keys[name] = field.state === 'empty' ? '' : field.text;
  }
  return keys;
}

/**
 * Exact drafts of several rows at once: one display window, one snapshot
 * read for the rows not yet staged. Staged inserts after the page resolve
 * to their staged row.
 */
export async function rowDrafts(
  args: DraftArgs,
  rows: readonly number[],
): Promise<Map<number, StagedEdit>> {
  const { info, result, history } = args;
  const drafts = new Map<number, StagedEdit>();
  const inserts = stagedInserts(history.current);
  const page = rows.filter((row) => row < result.rows);
  for (const row of rows.filter((item) => item >= result.rows)) {
    const insert = inserts[row - result.rows];
    if (!insert) throw new Error('That new row is no longer staged.');
    drafts.set(row, insert);
  }
  if (!page.length) return drafts;
  const fields = await readRows(result, page);
  const keyed = page.map((row) => {
    const keys = keysOf(fields.get(row) ?? [], info.key);
    return { row, keys, id: JSON.stringify(keys) };
  });
  const missing = keyed.filter(
    ({ id }) => !history.current.some((item) => item.id === id),
  );
  const snapshots = missing.length
    ? await tableEditRows(
        args.target,
        missing.map(({ keys }) => keys),
      )
    : [];
  keyed.forEach(({ row, id }) => {
    const existing = history.current.find((item) => item.id === id);
    const at = missing.findIndex((item) => item.id === id);
    const original = existing?.original ?? snapshots[at];
    if (!original) throw new Error('A selected row could not be read.');
    drafts.set(
      row,
      existing
        ? {
            ...existing,
            kind: existing.kind === 'delete' ? 'delete' : 'update',
          }
        : {
            id,
            number: args.offset + row + 1,
            kind: 'update',
            original,
            values: {},
          },
    );
  });
  return drafts;
}

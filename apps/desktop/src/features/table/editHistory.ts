// SPDX-License-Identifier: Apache-2.0
import type { RowEditDto } from '@openrows/ipc';
import { unchanged } from './cellValue';
/** Only staged rows live in React; the page remains in Arrow tiles. */
export type StagedEdit = RowEditDto & {
  id: string;
  number: number;
  /** Focus the field used to open the editor, without sending it to the server. */
  focusField?: string;
};
/** Staged items with undo/redo snapshots (rows here, structure changes too). */
export type History<T> = {
  pending: boolean;
  /** Lost COMMIT acknowledgement: reload before doing anything else. */
  uncertain?: boolean;
  current: readonly T[];
  past: readonly (readonly T[])[];
  future: readonly (readonly T[])[];
};
export type EditHistory = History<StagedEdit>;
/** Nothing staged; assignable to any `History<T>`. */
export const emptyHistory: History<never> = {
  pending: false,
  current: [],
  past: [],
  future: [],
};
/** Merge changes by stable original row identity; insert then delete cancels itself. */
export function stage(history: EditHistory, edit: StagedEdit): EditHistory {
  if (history.pending || history.uncertain) return history;
  return record(history, merge(history.current, edit));
}
/** Stage several edits (a bulk action) as one undoable step. */
export function stageMany(
  history: EditHistory,
  edits: readonly StagedEdit[],
): EditHistory {
  if (history.pending || history.uncertain || !edits.length) return history;
  return record(
    history,
    edits.reduce<readonly StagedEdit[]>(merge, history.current),
  );
}
/** Journaled edits from an earlier run; undo returns to nothing staged. */
export function restored(edits: readonly StagedEdit[]): EditHistory {
  return { pending: false, current: edits, past: [[]], future: [] };
}
function record(
  history: EditHistory,
  current: readonly StagedEdit[],
): EditHistory {
  return {
    pending: false,
    current,
    past: [...history.past.slice(-49), history.current],
    future: [],
  };
}
function merge(
  staged: readonly StagedEdit[],
  edit: StagedEdit,
): readonly StagedEdit[] {
  const existing = staged.find((item) => item.id === edit.id);
  const combined = existing
    ? {
        ...existing,
        kind:
          existing.kind === 'insert' && edit.kind !== 'delete'
            ? ('insert' as const)
            : edit.kind,
        values:
          edit.kind === 'delete' ? {} : { ...existing.values, ...edit.values },
      }
    : edit;
  if (combined.kind === 'update')
    combined.values = Object.fromEntries(
      Object.entries(combined.values).filter(
        ([name, value]) => !unchanged(value, combined.original[name]),
      ),
    );
  const keep =
    !(existing?.kind === 'insert' && edit.kind === 'delete') &&
    !(combined.kind === 'update' && !Object.keys(combined.values).length);
  const current = staged.filter((item) => item.id !== edit.id);
  if (keep) current.push(combined);
  return current;
}
/** Undo/redo affects staged values only, never database contents. */
export function travel<T>(
  history: History<T>,
  direction: 'undo' | 'redo',
): History<T> {
  if (history.pending || history.uncertain) return history;
  if (direction === 'undo') {
    const current = history.past.at(-1);
    return current
      ? {
          pending: false,
          current,
          past: history.past.slice(0, -1),
          future: [history.current, ...history.future],
        }
      : history;
  }
  const current = history.future[0];
  return current
    ? {
        pending: false,
        current,
        past: [...history.past, history.current],
        future: history.future.slice(1),
      }
    : history;
}

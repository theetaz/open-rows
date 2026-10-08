// SPDX-License-Identifier: Apache-2.0
import type { SavedEditsDto, StagedRowDto } from '@openrows/ipc';
import type { EditHistory, StagedEdit } from '../editHistory';

/** The journal payload of the staged rows: exact originals and values, no UI state. */
export function toSaved(
  history: EditHistory,
  key: readonly string[],
): SavedEditsDto {
  return {
    key: [...key],
    changes: history.current.map(
      ({ id, number, kind, original, values }): StagedRowDto => ({
        id,
        number,
        kind,
        original,
        values,
      }),
    ),
    savedAtMs: null,
  };
}

/** Journaled rows as staged edits; the optimistic guard still runs at commit. */
export function fromSaved(saved: SavedEditsDto): StagedEdit[] {
  return saved.changes.map(({ id, number, kind, original, values }) => ({
    id,
    number,
    kind,
    original,
    values,
  }));
}

/** The journal was staged against the same key columns as the table has now. */
export function sameKey(
  saved: readonly string[],
  current: readonly string[],
): boolean {
  return (
    saved.length === current.length &&
    saved.every((name, index) => name === current[index])
  );
}

/** `3 unsaved changes from Oct 8, 2:32 PM`. */
export function recoveryNote(count: number, savedAtMs: number | null): string {
  const changes = `${String(count)} unsaved ${count === 1 ? 'change' : 'changes'}`;
  if (savedAtMs === null) return changes;
  const when = new Intl.DateTimeFormat('en-US', {
    month: 'short',
    day: 'numeric',
    hour: 'numeric',
    minute: '2-digit',
  }).format(new Date(savedAtMs));
  return `${changes} from ${when}`;
}

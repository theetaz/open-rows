// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { emptyEdits, stage, travel } from './editHistory';
import type { StagedEdit } from './editHistory';
const edit: StagedEdit = {
  id: 'row-1',
  number: 1,
  kind: 'update',
  original: { id: '1', name: 'old', note: null },
  values: { name: 'new' },
};
it('merges updates, removes reverted cells, keeps exact originals and supports undo/redo', () => {
  const first = stage(emptyEdits, edit);
  const second = stage(first, { ...edit, values: { note: '' } });
  expect(second.current[0]?.values).toEqual({ name: 'new', note: '' });
  expect(travel(second, 'undo').current).toEqual(first.current);
  expect(travel(travel(second, 'undo'), 'redo').current).toEqual(
    second.current,
  );
  const reverted = stage(second, {
    ...edit,
    values: { name: 'old', note: null },
  });
  expect(reverted.current).toEqual([]);
  expect(first.current[0]?.values).toEqual({ name: 'new' });
});
it('collapses edit then delete; insert then delete cancels; pending commits cannot be changed', () => {
  const first = stage(emptyEdits, edit);
  const deleted = stage(first, { ...edit, kind: 'delete', values: {} });
  expect(deleted.current[0]?.kind).toBe('delete');
  expect(deleted.current[0]?.values).toEqual({});
  const restored = stage(deleted, edit);
  expect(restored.current[0]?.kind).toBe('update');
  const inserted = stage(emptyEdits, { ...edit, kind: 'insert', original: {} });
  expect(
    stage(inserted, { ...edit, kind: 'delete', values: {} }).current,
  ).toEqual([]);
  const pending = { ...first, pending: true };
  expect(stage(pending, edit)).toBe(pending);
  expect(travel(pending, 'undo')).toBe(pending);
});

// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { emptyHistory, stage, travel } from './editHistory';
import type { StagedEdit } from './editHistory';
import { DEFAULT_VALUE, NULL_VALUE, textValue } from './cellValue';
const edit: StagedEdit = {
  id: 'row-1',
  number: 1,
  kind: 'update',
  original: { id: '1', name: 'old', note: null },
  values: { name: textValue('new') },
};
it('merges updates, removes reverted cells, keeps exact originals and supports undo/redo', () => {
  const first = stage(emptyHistory, edit);
  const second = stage(first, { ...edit, values: { note: textValue('') } });
  expect(second.current[0]?.values).toEqual({
    name: textValue('new'),
    note: textValue(''),
  });
  expect(travel(second, 'undo').current).toEqual(first.current);
  expect(travel(travel(second, 'undo'), 'redo').current).toEqual(
    second.current,
  );
  const reverted = stage(second, {
    ...edit,
    values: { name: textValue('old'), note: NULL_VALUE },
  });
  expect(reverted.current).toEqual([]);
  expect(first.current[0]?.values).toEqual({ name: textValue('new') });
});
it('collapses edit then delete; insert then delete cancels; pending commits cannot be changed', () => {
  const first = stage(emptyHistory, edit);
  const deleted = stage(first, { ...edit, kind: 'delete', values: {} });
  expect(deleted.current[0]?.kind).toBe('delete');
  expect(deleted.current[0]?.values).toEqual({});
  const restored = stage(deleted, edit);
  expect(restored.current[0]?.kind).toBe('update');
  const inserted = stage(emptyHistory, {
    ...edit,
    kind: 'insert',
    original: {},
  });
  expect(
    stage(inserted, { ...edit, kind: 'delete', values: {} }).current,
  ).toEqual([]);
  const pending = { ...first, pending: true };
  expect(stage(pending, edit)).toBe(pending);
  expect(travel(pending, 'undo')).toBe(pending);
});
it('keeps NULL, EMPTY and DEFAULT distinct from each other and from the original', () => {
  const nullNote = stage(emptyHistory, {
    ...edit,
    values: { note: NULL_VALUE },
  });
  expect(nullNote.current).toEqual([]);
  const empty = stage(emptyHistory, {
    ...edit,
    values: { note: textValue('') },
  });
  expect(empty.current[0]?.values).toEqual({ note: textValue('') });
  const text = stage(emptyHistory, {
    ...edit,
    values: { note: textValue('NULL'), name: textValue('DEFAULT') },
  });
  expect(text.current[0]?.values).toEqual({
    note: textValue('NULL'),
    name: textValue('DEFAULT'),
  });
  const defaults = stage(emptyHistory, {
    ...edit,
    values: { name: DEFAULT_VALUE, note: DEFAULT_VALUE },
  });
  expect(defaults.current[0]?.values).toEqual({
    name: DEFAULT_VALUE,
    note: DEFAULT_VALUE,
  });
  const back = stage(defaults, { ...edit, values: { name: textValue('old') } });
  expect(back.current[0]?.values).toEqual({ note: DEFAULT_VALUE });
});

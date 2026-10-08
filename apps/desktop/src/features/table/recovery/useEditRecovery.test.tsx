// SPDX-License-Identifier: Apache-2.0
import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, expect, it, vi } from 'vitest';
import {
  clearTableEdits,
  savedTableEdits,
  saveTableEdits,
  tableEditInfo,
} from '@openrows/ipc';
import type { SavedEditsDto } from '@openrows/ipc';
import { textValue } from '../cellValue';
import { useTableViews } from '../useTableViews';
import { useRowEditing } from '../useRowEditing';
import { useEditRecovery } from './useEditRecovery';
import { recoveryNote } from './journal';

vi.mock('@openrows/ipc', async () =>
  (await import('../../../test/ipcStub')).ipcStub(),
);

const target = {
  profileId: 'synthetic',
  database: 'orders',
  schema: 'public',
  table: 'users',
};
const columns = [
  {
    name: 'id',
    dataType: 'integer',
    nullable: false,
    default: null,
    comment: null,
    primaryKey: true,
    enumLabels: null,
  },
  {
    name: 'name',
    dataType: 'text',
    nullable: true,
    default: null,
    comment: null,
    primaryKey: false,
    enumLabels: null,
  },
];
const saved: SavedEditsDto = {
  key: ['id'],
  savedAtMs: Date.UTC(2026, 9, 8, 12, 32),
  changes: [
    {
      id: '{"id":"7"}',
      number: 7,
      kind: 'update',
      original: { id: '7', name: 'old' },
      values: { name: { kind: 'text', value: 'Ada' } },
    },
    {
      id: 'new-row',
      number: 0,
      kind: 'insert',
      original: {},
      values: {},
    },
  ],
};

beforeEach(() => {
  vi.mocked(tableEditInfo).mockResolvedValue({
    key: ['id'],
    protected: [],
    columns,
    readOnly: false,
    confirmWrites: false,
    production: false,
  });
  vi.mocked(savedTableEdits).mockReset();
  vi.mocked(savedTableEdits).mockResolvedValue(null);
  vi.mocked(saveTableEdits).mockClear();
  vi.mocked(clearTableEdits).mockClear();
});

function mount() {
  return renderHook(() => {
    const views = useTableViews();
    const editing = useRowEditing({
      tabId: 't',
      target,
      sessionId: 's',
      offset: 0,
      views,
    });
    const recovery = useEditRecovery({ tabId: 't', target, views, editing });
    return { editing, recovery };
  });
}

it('journals staged edits on every mutation and clears the journal when nothing is staged', async () => {
  vi.mocked(savedTableEdits).mockResolvedValue(saved);
  const hook = mount();
  await waitFor(() => {
    expect(hook.result.current.editing.info?.key).toEqual(['id']);
  });
  expect(saveTableEdits).not.toHaveBeenCalled();
  expect(clearTableEdits).not.toHaveBeenCalled();
  act(() => {
    hook.result.current.editing.change({
      id: '{"id":"7"}',
      number: 7,
      kind: 'update',
      original: { id: '7', name: 'old' },
      values: { name: textValue('Ada') },
      focusField: 'name',
    });
  });
  await waitFor(() => {
    expect(saveTableEdits).toHaveBeenCalledTimes(1);
  });
  expect(vi.mocked(saveTableEdits).mock.calls[0]?.[1]).toEqual({
    key: ['id'],
    savedAtMs: null,
    changes: [
      {
        id: '{"id":"7"}',
        number: 7,
        kind: 'update',
        original: { id: '7', name: 'old' },
        values: { name: textValue('Ada') },
      },
    ],
  });
  act(() => {
    hook.result.current.editing.discard();
  });
  await waitFor(() => {
    expect(clearTableEdits).toHaveBeenCalledExactlyOnceWith(target);
  });
  // The earlier run's journal was superseded by this session's edits: no stale offer.
  expect(hook.result.current.recovery.offer).toBeNull();
});

it('offers journaled edits from an earlier run and restores them as one undoable step', async () => {
  vi.mocked(savedTableEdits).mockResolvedValue(saved);
  const hook = mount();
  await waitFor(() => {
    expect(hook.result.current.recovery.offer?.kind).toBe('edits');
  });
  await waitFor(() => {
    expect(hook.result.current.recovery.canRestore).toBe(true);
  });
  expect(recoveryNote(2, saved.savedAtMs)).toMatch(
    /^2 unsaved changes from Oct 8, \d{1,2}:\d{2} [AP]M$/,
  );
  act(() => {
    hook.result.current.recovery.restore();
  });
  expect(hook.result.current.recovery.offer).toBeNull();
  const { history } = hook.result.current.editing;
  expect(history.current).toHaveLength(2);
  expect(history.current[0]).toMatchObject({
    kind: 'update',
    original: { id: '7', name: 'old' },
    values: { name: textValue('Ada') },
  });
  expect(history.current[1]?.kind).toBe('insert');
  // Restoring re-journals the same rows, so the journal's time moves forward.
  await waitFor(() => {
    expect(saveTableEdits).toHaveBeenCalledTimes(1);
  });
  expect(vi.mocked(saveTableEdits).mock.calls[0]?.[1].changes).toHaveLength(2);
  act(() => {
    hook.result.current.editing.undo();
  });
  expect(hook.result.current.editing.history.current).toEqual([]);
  await waitFor(() => {
    expect(clearTableEdits).toHaveBeenCalledExactlyOnceWith(target);
  });
});

it('refuses to restore when the table key changed, and discard forgets the journal', async () => {
  vi.mocked(savedTableEdits).mockResolvedValue({ ...saved, key: ['uuid'] });
  const hook = mount();
  await waitFor(() => {
    expect(hook.result.current.editing.info?.key).toEqual(['id']);
    expect(hook.result.current.recovery.offer?.kind).toBe('edits');
  });
  expect(hook.result.current.recovery.canRestore).toBe(false);
  act(() => {
    hook.result.current.recovery.restore();
  });
  expect(hook.result.current.editing.history.current).toEqual([]);
  act(() => {
    hook.result.current.recovery.discard();
  });
  expect(hook.result.current.recovery.offer).toBeNull();
  expect(clearTableEdits).toHaveBeenCalledExactlyOnceWith(target);
});

it('reports an unreadable journal instead of hiding it', async () => {
  vi.mocked(savedTableEdits).mockRejectedValue(new Error('cannot be read'));
  const hook = mount();
  await waitFor(() => {
    expect(hook.result.current.recovery.offer).toEqual({
      kind: 'error',
      message: 'cannot be read',
    });
  });
  expect(hook.result.current.recovery.canRestore).toBe(false);
});

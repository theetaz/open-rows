// SPDX-License-Identifier: Apache-2.0
import { NULL_VALUE, textValue } from './cellValue';
import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, expect, it, vi } from 'vitest';
import { tableEditInfo, tableEditRow } from '@openrows/ipc';
import { useTableViews } from './useTableViews';
import { useRowEditing } from './useRowEditing';
vi.mock('@openrows/ipc', async () => ({
  ...(await import('../../test/ipcStub')).ipcStub(),
}));
vi.mock('../../results/displayWindow', () => ({
  displayWindow: vi.fn(() =>
    Promise.resolve({
      cells: [
        { text: '7', state: 'normal' },
        { text: 'display text', state: 'normal' },
      ],
      columns: 2,
      convertMs: 0,
    }),
  ),
}));
const result = {
  id: 'r',
  rows: 1,
  done: true,
  truncated: false,
  statement: 0,
  columns: [
    { name: 'id', sourceType: 'int4' },
    { name: 'name', sourceType: 'text' },
  ],
};
beforeEach(() => {
  vi.mocked(tableEditInfo).mockResolvedValue({
    key: ['id'],
    protected: [],
    readOnly: false,
    columns: ['id', 'name'].map((name) => ({
      name,
      dataType: name === 'id' ? 'integer' : 'text',
      nullable: name !== 'id',
      default: null,
      comment: null,
      primaryKey: name === 'id',
      enumLabels: null,
    })),
  });
  vi.mocked(tableEditRow).mockResolvedValue({ id: '7', name: 'exact' });
});
async function mount() {
  const hook = renderHook(() =>
    useRowEditing({
      tabId: 't',
      target: {
        profileId: 'p',
        database: 'd',
        schema: 'public',
        table: 'users',
      },
      sessionId: 's',
      offset: 0,
      views: useTableViews(),
    }),
  );
  await waitFor(() => {
    expect(hook.result.current.info).not.toBeNull();
  });
  return hook;
}

it('+ Row stages an empty insert, requests inline editing, and merges typed values into it', async () => {
  const hook = await mount();
  act(() => {
    hook.result.current.insert();
  });
  const [insert] = hook.result.current.history.current;
  expect(insert).toMatchObject({ kind: 'insert', values: {}, original: {} });
  expect(hook.result.current.inline).toEqual({ insert: insert?.id });
  expect(hook.result.current.draft).toBeNull();
  act(() => {
    const editing = hook.result.current;
    editing.stageCell(editing.draftFor(result, 1), 'name', textValue('Ada'));
  });
  await waitFor(() => {
    expect(hook.result.current.history.current[0]?.values).toEqual({
      name: textValue('Ada'),
    });
  });
  act(() => {
    const editing = hook.result.current;
    editing.stageCell(editing.draftFor(result, 1), 'name', NULL_VALUE);
  });
  await waitFor(() => {
    expect(hook.result.current.history.current[0]?.values).toEqual({
      name: NULL_VALUE,
    });
  });
  expect(hook.result.current.history.current).toHaveLength(1);
  act(() => {
    hook.result.current.quick(result, { row: 1, column: 1 }, 'delete');
  });
  await waitFor(() => {
    expect(hook.result.current.history.current).toHaveLength(0);
  });
  expect(tableEditRow).not.toHaveBeenCalled();
});

it('stages queued cell edits in order without one stale draft reverting another field', async () => {
  const hook = await mount();
  act(() => {
    const editing = hook.result.current;
    const early = editing.draftFor(result, 0);
    editing.stageCell(early, 'name', textValue('first'));
    editing.stageCell(editing.draftFor(result, 0), 'name', textValue('second'));
    editing.stageCell(early, 'id', textValue('8'));
  });
  await waitFor(() => {
    expect(hook.result.current.history.current[0]?.values).toEqual({
      name: textValue('second'),
      id: textValue('8'),
    });
  });
  const edit = hook.result.current.history.current[0];
  expect(edit?.original).toEqual({ id: '7', name: 'exact' });
  act(() => {
    const editing = hook.result.current;
    editing.stageCell(editing.draftFor(result, 0), 'name', textValue('exact'));
    editing.stageCell(editing.draftFor(result, 0), 'id', textValue('7'));
  });
  await waitFor(() => {
    expect(hook.result.current.history.current).toHaveLength(0);
  });
});

it('shows why a row cannot be read and stages nothing', async () => {
  const hook = await mount();
  vi.mocked(tableEditRow).mockRejectedValueOnce(new Error('Row is gone'));
  act(() => {
    const editing = hook.result.current;
    editing.stageCell(editing.draftFor(result, 0), 'name', textValue('lost'));
  });
  await waitFor(() => {
    expect(hook.result.current.error).toBe('Row is gone');
  });
  expect(hook.result.current.history.current).toHaveLength(0);
});

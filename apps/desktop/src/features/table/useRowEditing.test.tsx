// SPDX-License-Identifier: Apache-2.0
import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, expect, it, vi } from 'vitest';
import {
  CommandError,
  tableEdit,
  tableEditInfo,
  tableEditRow,
} from '@openrows/ipc';
import { useTableViews } from './useTableViews';
import { useRowEditing } from './useRowEditing';
vi.mock('@openrows/ipc', async () => {
  const actual =
    await vi.importActual<typeof import('@openrows/ipc')>('@openrows/ipc');
  return {
    ...(await import('../../test/ipcStub')).ipcStub(),
    CommandError: actual.CommandError,
  };
});
vi.mock('../../results/displayWindow', () => ({
  displayWindow: vi.fn(() =>
    Promise.resolve({
      cells: [
        { text: '7', state: 'normal' },
        { text: 'rounded display', state: 'normal' },
      ],
      columns: 2,
      convertMs: 0,
    }),
  ),
}));
const target = {
  profileId: 'synthetic',
  database: 'orders',
  schema: 'public',
  table: 'users',
};
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
const columns = [
  {
    name: 'id',
    dataType: 'integer',
    nullable: false,
    default: null,
    comment: null,
  },
  {
    name: 'name',
    dataType: 'text',
    nullable: true,
    default: null,
    comment: null,
  },
];
beforeEach(() => {
  vi.mocked(tableEditInfo).mockResolvedValue({
    key: ['id'],
    protected: [],
    columns,
    readOnly: false,
  });
  vi.mocked(tableEditRow).mockResolvedValue({
    id: '7',
    name: 'exact original',
  });
  vi.mocked(tableEdit).mockReset();
});
function mount() {
  return renderHook(() => {
    const views = useTableViews();
    return useRowEditing({
      tabId: 't',
      target,
      sessionId: 's',
      offset: 300,
      views,
    });
  });
}
it('stages exact snapshots, reviews first, keeps changes after conflicts, and reloads after commit', async () => {
  const hook = mount();
  await waitFor(() => {
    expect(hook.result.current.info?.key).toEqual(['id']);
  });
  act(() => {
    hook.result.current.open(result, { row: 0, column: 1 });
  });
  await waitFor(() => {
    expect(hook.result.current.draft?.original.name).toBe('exact original');
  });
  expect(hook.result.current.draft?.number).toBe(301);
  expect(hook.result.current.draft?.focusField).toBe('name');
  act(() => {
    const draft = hook.result.current.draft;
    if (draft) hook.result.current.change({ ...draft, values: { name: null } });
  });
  vi.mocked(tableEdit).mockResolvedValueOnce({
    kind: 'prepared',
    token: 'reviewed',
    statements: [{ sql: 'UPDATE', params: [null] }],
  });
  act(() => {
    hook.result.current.commit();
  });
  await waitFor(() => {
    expect(hook.result.current.review?.token).toBe('reviewed');
  });
  expect(vi.mocked(tableEdit).mock.calls[0]?.[0].confirmToken).toBeNull();
  vi.mocked(tableEdit).mockRejectedValueOnce(
    new Error('Conflict; rolled back'),
  );
  act(() => {
    hook.result.current.commit('reviewed');
  });
  await waitFor(() => {
    expect(hook.result.current.error).toContain('Conflict');
  });
  expect(hook.result.current.history.current).toHaveLength(1);
  vi.mocked(tableEdit).mockResolvedValueOnce({ kind: 'committed' });
  act(() => {
    hook.result.current.commit('reviewed');
  });
  await waitFor(() => {
    expect(hook.result.current.history.current).toHaveLength(0);
  });
  expect(vi.mocked(tableEdit).mock.calls[2]?.[0].confirmToken).toBe('reviewed');
});
it('does not expose row editing on read-only connections or when key columns are hidden', async () => {
  vi.mocked(tableEditInfo).mockResolvedValueOnce({
    key: ['id'],
    protected: [],
    columns,
    readOnly: true,
  });
  const hook = mount();
  await waitFor(() => {
    expect(hook.result.current.info?.readOnly).toBe(true);
  });
  act(() => {
    hook.result.current.open(result, { row: 0, column: 0 });
  });
  expect(hook.result.current.draft).toBeNull();
  hook.unmount();
  const writable = mount();
  await waitFor(() => {
    expect(writable.result.current.info?.key).toEqual(['id']);
  });
  act(() => {
    writable.result.current.open(
      {
        ...result,
        columns: [result.columns[1] as { name: string; sourceType: string }],
      },
      { row: 0, column: 0 },
    );
  });
  await waitFor(() => {
    expect(writable.result.current.error).toContain('unique-key');
  });
});

it('retains uncertain commits and prevents retry until reload/discard', async () => {
  const hook = mount();
  await waitFor(() => {
    expect(hook.result.current.info?.key).toEqual(['id']);
  });
  act(() => {
    hook.result.current.change({
      id: 'new',
      number: 0,
      kind: 'insert',
      original: {},
      values: { name: 'new' },
    });
  });
  vi.mocked(tableEdit).mockRejectedValueOnce(
    new CommandError({
      kind: 'server',
      code: '08007',
      message: 'Commit outcome unknown',
    }),
  );
  act(() => {
    hook.result.current.commit('reviewed');
  });
  await waitFor(() => {
    expect(hook.result.current.history.uncertain).toBe(true);
  });
  act(() => {
    hook.result.current.commit('reviewed');
  });
  expect(tableEdit).toHaveBeenCalledOnce();
  expect(hook.result.current.history.current).toHaveLength(1);
  act(() => {
    hook.result.current.discard();
  });
  expect(hook.result.current.history.uncertain).toBeUndefined();
  expect(hook.result.current.history.current).toHaveLength(0);
});
it('quick context actions use exact snapshots and only stage values, duplicates or deletions', async () => {
  const hook = mount();
  await waitFor(() => {
    expect(hook.result.current.info?.key).toEqual(['id']);
  });
  act(() => {
    hook.result.current.quick(result, { row: 0, column: 1 }, 'empty');
  });
  await waitFor(() => {
    expect(hook.result.current.history.current[0]?.values.name).toBe('');
  });
  expect(hook.result.current.history.current[0]?.original.name).toBe(
    'exact original',
  );
  act(() => {
    hook.result.current.quick(result, { row: 0, column: 1 }, 'null');
  });
  await waitFor(() => {
    expect(hook.result.current.history.current[0]?.values.name).toBeNull();
  });
  act(() => {
    hook.result.current.quick(result, { row: 0, column: 1 }, 'duplicate');
  });
  await waitFor(() => {
    expect(hook.result.current.history.current).toHaveLength(2);
  });
  const duplicate = hook.result.current.history.current.find(
    (edit) => edit.kind === 'insert',
  );
  expect(duplicate?.values).toEqual({ name: null });
  act(() => {
    hook.result.current.quick(result, { row: 0, column: 1 }, 'delete');
  });
  await waitFor(() => {
    expect(
      hook.result.current.history.current.find(
        (edit) => edit.kind === 'delete',
      ),
    ).toBeDefined();
  });
  expect(tableEdit).not.toHaveBeenCalled();
  expect(tableEditRow).toHaveBeenCalledWith(target, { id: '7' });
});

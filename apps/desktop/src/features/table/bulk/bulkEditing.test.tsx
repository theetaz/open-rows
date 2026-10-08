// SPDX-License-Identifier: Apache-2.0
import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, expect, it, vi } from 'vitest';
import { tableEditInfo, tableEditRows } from '@openrows/ipc';
import type { WindowRequest } from '@openrows/ipc';
import { NULL_VALUE, textValue } from '../cellValue';
import { useTableViews } from '../useTableViews';
import { useRowEditing } from '../useRowEditing';

vi.mock('@openrows/ipc', async () =>
  (await import('../../../test/ipcStub')).ipcStub(),
);
vi.mock('../../../results/displayWindow', () => ({
  displayWindow: vi.fn((request: WindowRequest) =>
    Promise.resolve({
      cells: Array.from({ length: request.rowCount }, (_, row) => [
        { text: String(7 + request.rowStart + row), state: 'normal' },
        { text: `name ${String(request.rowStart + row)}`, state: 'normal' },
      ]).flat(),
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
  rows: 3,
  done: true,
  truncated: false,
  statement: 0,
  columns: [
    { name: 'id', sourceType: 'int4' },
    { name: 'name', sourceType: 'text' },
  ],
};
const facts = { comment: null, primaryKey: false, enumLabels: null };
const columns = [
  {
    ...facts,
    name: 'id',
    dataType: 'integer',
    nullable: false,
    default: null,
    primaryKey: true,
  },
  { ...facts, name: 'name', dataType: 'text', nullable: true, default: null },
];
const names = { r0: 0, r1: 1, c0: 1, c1: 1 };

beforeEach(() => {
  vi.mocked(tableEditInfo).mockResolvedValue({
    key: ['id'],
    protected: [],
    columns,
    readOnly: false,
    confirmWrites: false,
    production: false,
  });
  vi.mocked(tableEditRows).mockReset();
  vi.mocked(tableEditRows).mockImplementation((_target, keys) =>
    Promise.resolve(
      keys.map((key) => ({ id: key.id ?? '', name: `exact ${key.id ?? ''}` })),
    ),
  );
});

async function mount() {
  const hook = renderHook(() => {
    const views = useTableViews();
    return useRowEditing({
      tabId: 't',
      target,
      sessionId: 's',
      offset: 0,
      views,
    });
  });
  await waitFor(() => {
    expect(hook.result.current.info?.key).toEqual(['id']);
  });
  return hook;
}

it('sets a value in every selected cell with one snapshot read and one undo step', async () => {
  const hook = await mount();
  act(() => {
    hook.result.current.bulk.set(result, [names], NULL_VALUE);
  });
  await waitFor(() => {
    expect(hook.result.current.history.current).toHaveLength(2);
  });
  expect(tableEditRows).toHaveBeenCalledExactlyOnceWith(target, [
    { id: '7' },
    { id: '8' },
  ]);
  const [first, second] = hook.result.current.history.current;
  expect(first).toMatchObject({
    id: '{"id":"7"}',
    number: 1,
    kind: 'update',
    original: { id: '7', name: 'exact 7' },
    values: { name: NULL_VALUE },
  });
  expect(second?.number).toBe(2);
  expect(hook.result.current.history.past).toHaveLength(1);
  act(() => {
    hook.result.current.undo();
  });
  expect(hook.result.current.history.current).toEqual([]);
});

it('validates every cell first and stages nothing when one refuses the value', async () => {
  const hook = await mount();
  act(() => {
    hook.result.current.bulk.set(
      result,
      [{ r0: 0, r1: 2, c0: 0, c1: 1 }],
      textValue('abc'),
    );
  });
  await waitFor(() => {
    expect(hook.result.current.error).toContain('is not an integer');
  });
  expect(hook.result.current.error).toContain('Nothing was staged');
  expect(hook.result.current.error).toContain('Row 3 · id');
  expect(hook.result.current.history.current).toEqual([]);
  act(() => {
    hook.result.current.bulk.set(result, [names], NULL_VALUE);
  });
  await waitFor(() => {
    expect(hook.result.current.history.current).toHaveLength(2);
  });
});

it('pastes one value into the selection and a TSV block from its top-left cell', async () => {
  const hook = await mount();
  act(() => {
    hook.result.current.bulk.paste(result, [names], 'NULL');
  });
  await waitFor(() => {
    expect(hook.result.current.history.current).toHaveLength(2);
  });
  expect(
    hook.result.current.history.current.map((edit) => edit.values),
  ).toEqual([{ name: NULL_VALUE }, { name: NULL_VALUE }]);
  act(() => {
    hook.result.current.bulk.paste(
      result,
      [{ r0: 1, r1: 1, c0: 0, c1: 0 }],
      '70\tAda\n80\t"Bob\tB"\n90\tbeyond\n',
    );
  });
  await waitFor(() => {
    expect(hook.result.current.history.past).toHaveLength(2);
  });
  const byId = Object.fromEntries(
    hook.result.current.history.current.map((edit) => [edit.id, edit.values]),
  );
  expect(byId['{"id":"8"}']).toEqual({
    id: textValue('70'),
    name: textValue('Ada'),
  });
  expect(byId['{"id":"9"}']).toEqual({
    id: textValue('80'),
    name: textValue('Bob\tB'),
  });
  expect(Object.keys(byId)).toHaveLength(3);
});

it('deletes and duplicates every selected row as one step each', async () => {
  const hook = await mount();
  act(() => {
    hook.result.current.bulk.deleteRows(result, [names]);
  });
  await waitFor(() => {
    expect(hook.result.current.history.current).toHaveLength(2);
  });
  expect(
    hook.result.current.history.current.every(
      (edit) => edit.kind === 'delete' && !Object.keys(edit.values).length,
    ),
  ).toBe(true);
  act(() => {
    hook.result.current.bulk.duplicateRows(result, [
      { r0: 2, r1: 2, c0: 0, c1: 0 },
    ]);
  });
  await waitFor(() => {
    expect(hook.result.current.history.current).toHaveLength(3);
  });
  const copy = hook.result.current.history.current.at(-1);
  expect(copy?.kind).toBe('insert');
  expect(copy?.values).toEqual({ name: textValue('exact 9') });
  expect(hook.result.current.history.past).toHaveLength(2);
});

it('does nothing on read-only connections', async () => {
  vi.mocked(tableEditInfo).mockResolvedValueOnce({
    key: ['id'],
    protected: [],
    columns,
    readOnly: true,
    confirmWrites: false,
    production: false,
  });
  const hook = await mount();
  act(() => {
    hook.result.current.bulk.set(result, [names], NULL_VALUE);
    hook.result.current.bulk.paste(result, [names], 'x');
    hook.result.current.bulk.deleteRows(result, [names]);
    hook.result.current.bulk.duplicateRows(result, [names]);
  });
  await act(() => Promise.resolve());
  expect(tableEditRows).not.toHaveBeenCalled();
  expect(hook.result.current.history.current).toEqual([]);
});

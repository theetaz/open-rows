// SPDX-License-Identifier: Apache-2.0
import { act, renderHook } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import { runTableCommand } from '../commands';
import type { ResultMeta } from '../query/run';
import { selectionText } from './copySelection';
import type { RowField } from './row';
import type { RowEditing } from './useRowEditing';
import { useTableCommandTarget } from './useTableCommandTarget';
import type { TableTabState } from './useTableTab';

vi.mock('./row', async (original) => ({
  ...(await original<typeof import('./row')>()),
  readRows: vi.fn((_: unknown, rows: readonly number[]) =>
    Promise.resolve(
      new Map(
        rows.map((row) => [
          row,
          ['id', 'name', 'note'].map((name, index): RowField => ({
            name,
            index,
            type: 'text',
            text: index === 2 ? 'a\tb' : `${name}${String(row)}`,
            state: 'normal',
          })),
        ]),
      ),
    ),
  ),
}));

const result = {
  id: 'page-1',
  rows: 3,
  columns: [{ name: 'id' }, { name: 'name' }, { name: 'note' }],
} as unknown as ResultMeta;

function editing(staged: number, writable = true): RowEditing {
  return {
    info: { readOnly: !writable, key: ['id'], rowId: null },
    history: { current: Array.from({ length: staged }), uncertain: false },
    busy: false,
    commit: vi.fn(),
    discard: vi.fn(),
    insert: vi.fn(),
    quick: vi.fn(),
    fail: vi.fn(),
    bulk: { deleteRows: vi.fn() },
  } as unknown as RowEditing;
}

function state(mode: 'data' | 'columns' = 'data'): TableTabState {
  return {
    view: { mode },
    result,
    actions: {
      toggleColumns: vi.fn(),
      reload: vi.fn(),
      mode: vi.fn(),
      toggleFilters: vi.fn(),
    },
  } as unknown as TableTabState;
}

function mount(rows: RowEditing, view = state()) {
  const root = { current: null };
  return renderHook(() =>
    useTableCommandTarget({ root, state: view, editing: rows, ready: true }),
  );
}

it('commits and discards only staged changes of the data view', () => {
  const clean = editing(0);
  const cleanView = mount(clean);
  runTableCommand('commit-changes');
  runTableCommand('discard-changes');
  expect(clean.commit).not.toHaveBeenCalled();
  expect(clean.discard).not.toHaveBeenCalled();
  cleanView.unmount();
  const staged = editing(2);
  const view = mount(staged);
  runTableCommand('commit-changes');
  runTableCommand('discard-changes');
  expect(staged.commit).toHaveBeenCalledOnce();
  expect(staged.discard).toHaveBeenCalledOnce();
  view.unmount();
  const readOnly = editing(2, false);
  mount(readOnly);
  runTableCommand('commit-changes');
  runTableCommand('insert-row');
  expect(readOnly.commit).not.toHaveBeenCalled();
  expect(readOnly.insert).not.toHaveBeenCalled();
});

it('inserts, deletes the selection or focused row, toggles columns and reloads', () => {
  const rows = editing(0);
  const view = state();
  const hook = mount(rows, view);
  runTableCommand('insert-row');
  expect(rows.insert).toHaveBeenCalledOnce();
  runTableCommand('delete-row');
  expect(rows.quick).not.toHaveBeenCalled();
  act(() => {
    hook.result.current({ result, cell: { row: 1, column: 0 }, ranges: [] });
  });
  runTableCommand('delete-row');
  expect(rows.quick).toHaveBeenCalledWith(
    result,
    { row: 1, column: 0 },
    'delete',
  );
  const ranges = [{ r0: 0, r1: 2, c0: 0, c1: 0 }];
  hook.result.current({ result, cell: { row: 0, column: 0 }, ranges });
  runTableCommand('delete-row');
  expect(rows.bulk.deleteRows).toHaveBeenCalledWith(result, ranges);
  runTableCommand('toggle-columns');
  runTableCommand('refresh');
  expect(view.actions.toggleColumns).toHaveBeenCalledOnce();
  expect(view.actions.reload).toHaveBeenCalledOnce();
});

it('copies selected cells as TSV, or the focused row', async () => {
  expect(
    await selectionText({
      result,
      cell: { row: 0, column: 0 },
      ranges: [{ r0: 0, r1: 1, c0: 1, c1: 2 }],
    }),
  ).toBe('name0\t"a\tb"\nname1\t"a\tb"');
  expect(
    await selectionText({ result, cell: { row: 2, column: 1 }, ranges: [] }),
  ).toBe('id2\tname2\t"a\tb"');
});

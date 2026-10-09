// SPDX-License-Identifier: Apache-2.0
import { act, renderHook } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { runTableCommand } from '../commands';
import type { ResultMeta } from '../query/run';
import type { WindowRequest } from '@openrows/ipc';
import { emptyLayout } from '../../results/columnLayout';
import type { RowEditing } from './useRowEditing';
import { useTableCommandTarget } from './useTableCommandTarget';
import type { TableTabState } from './useTableTab';

vi.mock('../../results/displayWindow', () => ({
  displayWindow: vi.fn((request: WindowRequest) => {
    const cells = [];
    for (let row = 0; row < request.rowCount; row++)
      for (let column = 0; column < request.colCount; column++) {
        const at = request.colStart + column,
          name = ['id', 'name', 'note'][at] ?? '';
        cells.push({
          text: at === 2 ? 'a\tb' : `${name}${String(request.rowStart + row)}`,
          state: 'normal' as const,
        });
      }
    return Promise.resolve({ cells, columns: request.colCount, convertMs: 0 });
  }),
}));

const result = {
  id: 'page-1',
  rows: 3,
  columns: [
    { name: 'id', sourceType: 'int4' },
    { name: 'name', sourceType: 'text' },
    { name: 'note', sourceType: 'text' },
  ],
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
    layout: { change: vi.fn() },
    menu: {
      grid: {
        copyTarget: {
          engine: 'postgres',
          table: { schema: 'public', name: 'notes' },
        },
        pinned: [],
        togglePin: vi.fn(),
      },
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

it('copies selected cells as TSV or another format, or the focused row', async () => {
  // user-event installs the clipboard stub that jsdom lacks.
  userEvent.setup();
  const write = vi.spyOn(navigator.clipboard, 'writeText').mockResolvedValue();
  const hook = mount(editing(0));
  act(() => {
    hook.result.current({
      result,
      cell: { row: 0, column: 0 },
      ranges: [{ r0: 0, r1: 1, c0: 1, c1: 2 }],
    });
  });
  runTableCommand('copy-selection');
  await vi.waitFor(() => {
    expect(write).toHaveBeenLastCalledWith('name0\t"a\tb"\nname1\t"a\tb"');
  });
  runTableCommand('copy-as-insert');
  await vi.waitFor(() => {
    expect(write).toHaveBeenLastCalledWith(
      `INSERT INTO "public"."notes" ("name", "note") VALUES ('name0', 'a\tb');\nINSERT INTO "public"."notes" ("name", "note") VALUES ('name1', 'a\tb');`,
    );
  });
  act(() => {
    hook.result.current({ result, cell: { row: 2, column: 1 }, ranges: [] });
  });
  runTableCommand('copy-selection');
  await vi.waitFor(() => {
    expect(write).toHaveBeenLastCalledWith('id2\tname2\t"a\tb"');
  });
});

it('pins the focused column and resets the column layout', () => {
  const view = state();
  const hook = mount(editing(0), view);
  act(() => {
    hook.result.current({ result, cell: { row: 0, column: 1 }, ranges: [] });
  });
  runTableCommand('pin-column');
  expect(view.menu.grid?.togglePin).toHaveBeenCalledWith('name');
  runTableCommand('reset-columns');
  const reset = vi.mocked(view.layout.change).mock.calls[0]?.[0];
  expect(reset?.({ widths: { a: 90 }, order: ['a'], pinned: [] })).toBe(
    emptyLayout,
  );
});

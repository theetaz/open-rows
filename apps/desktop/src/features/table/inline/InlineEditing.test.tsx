// SPDX-License-Identifier: Apache-2.0
import { fromOriginal, NULL_VALUE, textValue } from '../cellValue';
import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import { mountGrid } from '@openrows/grid/canvas';
import type { GridHandle, GridProps } from '@openrows/grid';
import type { TableColumnDto } from '@openrows/ipc';
import { TableGrid } from '../TableGrid';
import { emptyHistory } from '../editHistory';
import type { StagedEdit } from '../editHistory';
import type { InlineRequest, RowEditing } from '../useRowEditing';

const announce = vi.fn(),
  focus = vi.fn(),
  select = vi.fn(),
  refresh = vi.fn();
const handle = {
  metrics: {},
  viewport: { x: 0, y: 0, width: 800, height: 400 },
  scrollTo: vi.fn(),
  destroy: vi.fn(),
  refresh,
  cellRect: () => ({ x: 96, y: 26, width: 180, height: 26 }),
  select,
  focus,
  announce,
} as unknown as GridHandle;
vi.mock('@openrows/grid/canvas', () => ({ mountGrid: vi.fn(() => handle) }));

const json = JSON.stringify({ items: Array.from({ length: 12 }, (_, i) => i) });
const longJson = `{"note":"${'x'.repeat(130)}"}`;
const draft: StagedEdit = {
  id: '{"id":"7"}',
  number: 1,
  kind: 'update',
  original: { id: '7', name: 'exact', done: 'true', mood: 'sad', doc: json },
  values: {},
};
const pending = Promise.resolve(draft);
const result = {
  id: 'page',
  rows: 1,
  done: true,
  truncated: false,
  statement: 0,
  columns: [
    { name: 'id', sourceType: 'int4' },
    { name: 'name', sourceType: 'text' },
    { name: 'done', sourceType: 'bool' },
    { name: 'mood', sourceType: 'mood' },
    { name: 'doc', sourceType: 'jsonb' },
  ],
};
function column(
  name: string,
  dataType: string,
  extra: Partial<TableColumnDto> = {},
): TableColumnDto {
  return {
    name,
    dataType,
    nullable: true,
    default: null,
    comment: null,
    primaryKey: false,
    enumLabels: null,
    ...extra,
  };
}
function editing(overrides: Partial<RowEditing> = {}): RowEditing {
  return {
    info: {
      key: ['id'],
      protected: ['id'],
      rowId: null,
      readOnly: false,
      confirmWrites: false,
      production: false,
      columns: [
        column('id', 'integer', { nullable: false, primaryKey: true }),
        column('name', 'text'),
        column('done', 'boolean'),
        column('mood', 'mood', {
          nullable: false,
          enumLabels: ['sad', 'happy'],
        }),
        column('doc', 'jsonb'),
      ],
    },
    history: emptyHistory,
    inline: null,
    clearInline: vi.fn(),
    draftFor: vi.fn(() => pending),
    stageCell: vi.fn(),
    bulk: { set: vi.fn(), paste: vi.fn() },
    ...overrides,
  } as unknown as RowEditing;
}
beforeEach(() => {
  vi.clearAllMocks();
});
async function mount(editor: RowEditing): Promise<GridProps> {
  render(
    <ThemeProvider>
      <TableGrid
        result={result}
        sort={null}
        editing={editor}
        offset={0}
        rowNumbers={false}
        striped
        onHeader={vi.fn()}
        onRow={vi.fn()}
        onContextMenu={vi.fn()}
      />
    </ThemeProvider>,
  );
  await waitFor(() => {
    expect(mountGrid).toHaveBeenCalled();
  });
  return vi.mocked(mountGrid).mock.calls[0]?.[1] as GridProps;
}
async function openName(grid: GridProps): Promise<HTMLInputElement> {
  act(() => {
    grid.onEdit?.({ row: 0, column: 1 });
  });
  const input = screen.getByRole<HTMLInputElement>('textbox', {
    name: 'Edit name',
  });
  await waitFor(() => {
    expect(input).toHaveValue('exact');
  });
  return input;
}

it('edits in place over the cell: Enter stages the exact field, unchanged Enter stages nothing', async () => {
  const editor = editing(),
    user = userEvent.setup();
  const grid = await mount(editor);
  const input = await openName(grid);
  expect(input).toHaveFocus();
  expect(input.style.left).toBe('96px');
  expect(input.style.width).toBe('180px');
  expect(announce).toHaveBeenCalledWith('Editing name');
  await user.keyboard('{Enter}');
  expect(editor.stageCell).not.toHaveBeenCalled();
  await openName(grid);
  await user.clear(screen.getByRole('textbox', { name: 'Edit name' }));
  await user.keyboard('renamed{Enter}');
  expect(editor.draftFor).toHaveBeenCalledWith(result, 0);
  expect(editor.stageCell).toHaveBeenCalledExactlyOnceWith(
    pending,
    'name',
    textValue('renamed'),
  );
  expect(screen.queryByRole('textbox', { name: 'Edit name' })).toBeNull();
  expect(focus).toHaveBeenCalled();
});

it('Escape cancels without staging and returns focus to the grid', async () => {
  const editor = editing(),
    user = userEvent.setup();
  const grid = await mount(editor);
  await openName(grid);
  await user.keyboard('{End} changed{Escape}');
  expect(screen.queryByRole('textbox', { name: 'Edit name' })).toBeNull();
  expect(editor.stageCell).not.toHaveBeenCalled();
  expect(focus).toHaveBeenCalled();
  expect(announce).toHaveBeenLastCalledWith('Edit cancelled');
});

it('Tab, Shift+Tab and arrows stage and move; ⌘⌫ stages NULL; a typed key seeds the value', async () => {
  const editor = editing(),
    user = userEvent.setup();
  const grid = await mount(editor);
  await openName(grid);
  await user.keyboard('!{Tab}');
  expect(editor.stageCell).toHaveBeenLastCalledWith(
    pending,
    'name',
    textValue('exact!'),
  );
  expect(select).toHaveBeenLastCalledWith(0, 2);
  await openName(grid);
  await user.keyboard('?{Shift>}{Tab}{/Shift}');
  expect(select).toHaveBeenLastCalledWith(0, 0);
  await openName(grid);
  await user.keyboard('{Meta>}{Backspace}{/Meta}');
  expect(editor.stageCell).toHaveBeenLastCalledWith(
    pending,
    'name',
    NULL_VALUE,
  );
  act(() => {
    grid.onEdit?.({ row: 0, column: 1 }, 'z');
  });
  const seeded = screen.getByRole('textbox', { name: 'Edit name' });
  expect(seeded).toHaveValue('z');
  await act(() => pending);
  expect(seeded).toHaveValue('z');
  await user.keyboard('{ArrowDown}');
  expect(editor.stageCell).toHaveBeenLastCalledWith(
    pending,
    'name',
    textValue('z'),
  );
  expect(select).toHaveBeenLastCalledWith(0, 1);
});

it('boolean cells open a TRUE / FALSE / NULL menu that stages the choice', async () => {
  const editor = editing(),
    user = userEvent.setup();
  const grid = await mount(editor);
  for (const [label, value] of [
    ['FALSE', 'false'],
    ['NULL', null],
  ] as const) {
    act(() => {
      grid.onEdit?.({ row: 0, column: 2 });
    });
    const menu = screen.getByRole('menu', { name: 'Values for done' });
    expect(
      Array.from(
        menu.querySelectorAll('[role=menuitem]'),
        (item) => item.textContent,
      ),
    ).toEqual(['TRUE', 'FALSE', 'NULL']);
    await user.click(screen.getByRole('menuitem', { name: label }));
    expect(editor.stageCell).toHaveBeenLastCalledWith(
      pending,
      'done',
      fromOriginal(value),
    );
  }
  act(() => {
    grid.onEdit?.({ row: 0, column: 2 });
  });
  await user.keyboard('{Escape}');
  expect(editor.stageCell).toHaveBeenCalledTimes(2);
});

it('enum cells list their labels, without NULL for NOT NULL columns', async () => {
  const editor = editing(),
    user = userEvent.setup();
  const grid = await mount(editor);
  await waitFor(() => {
    expect(refresh).toHaveBeenCalled();
  });
  expect(grid.source.columns[3]?.kind).toBe('enum');
  act(() => {
    grid.onEdit?.({ row: 0, column: 3 });
  });
  expect(screen.queryByRole('menuitem', { name: 'NULL' })).toBeNull();
  await user.click(screen.getByRole('menuitem', { name: 'happy' }));
  expect(editor.stageCell).toHaveBeenLastCalledWith(
    pending,
    'mood',
    textValue('happy'),
  );
});

it('long JSON opens a popover that pretty-prints and saves with ⌘Enter', async () => {
  const editor = editing({
      draftFor: vi.fn(() =>
        Promise.resolve({
          ...draft,
          original: { ...draft.original, doc: longJson },
        }),
      ),
    }),
    user = userEvent.setup();
  const grid = await mount(editor);
  act(() => {
    grid.onEdit?.({ row: 0, column: 4 });
  });
  const dialog = await screen.findByRole('dialog', { name: 'Edit doc' });
  const text = screen.getByRole('textbox', { name: 'Value of doc' });
  await waitFor(() => {
    expect(text).toHaveValue(longJson);
  });
  expect(dialog).toBeInTheDocument();
  await user.click(screen.getByRole('button', { name: 'Pretty' }));
  const pretty = JSON.stringify(JSON.parse(longJson), null, 2);
  expect(text).toHaveValue(pretty);
  await user.click(text);
  await user.keyboard('{Enter}');
  expect(editor.stageCell).not.toHaveBeenCalled();
  await user.keyboard('{Meta>}{Enter}{/Meta}');
  expect(editor.stageCell).toHaveBeenCalledExactlyOnceWith(
    expect.any(Promise),
    'doc',
    textValue(`${pretty}\n`),
  );
  expect(screen.queryByRole('dialog')).toBeNull();
});

it('does not open an editor on read-only tables or generated columns', async () => {
  const readOnly = editing();
  if (readOnly.info) readOnly.info.readOnly = true;
  const grid = await mount(readOnly);
  act(() => {
    grid.onEdit?.({ row: 0, column: 1 });
  });
  expect(screen.queryByRole('textbox')).toBeNull();
  expect(announce).toHaveBeenCalledWith('Rows cannot be edited here');
  expect(readOnly.draftFor).not.toHaveBeenCalled();
});

it('+ Row selects the staged insert after the page and edits its first editable cell', async () => {
  const insert: StagedEdit = {
    id: 'new-row',
    number: 0,
    kind: 'insert',
    original: {},
    values: {},
  };
  const request: InlineRequest = { insert: 'new-row' };
  const editor = editing({
    inline: request,
    history: { ...emptyHistory, current: [insert] },
    draftFor: vi.fn(() => Promise.resolve(insert)),
  });
  const user = userEvent.setup();
  const grid = await mount(editor);
  expect(grid.source.rows).toBe(2);
  await waitFor(() => {
    expect(editor.clearInline).toHaveBeenCalled();
  });
  expect(select).toHaveBeenCalledWith(1, 1);
  expect(editor.draftFor).toHaveBeenCalledWith(result, 1);
  const input = await screen.findByRole('textbox', { name: 'Edit name' });
  await waitFor(() => {
    expect(input).not.toHaveAttribute('readonly');
  });
  await user.keyboard('Ada{Enter}');
  expect(editor.stageCell).toHaveBeenCalledExactlyOnceWith(
    expect.any(Promise),
    'name',
    textValue('Ada'),
  );
});

it('typing into a multi-cell selection stages the value in every selected cell, and ⌘V pastes into it', async () => {
  const editor = editing(),
    user = userEvent.setup();
  const grid = await mount(editor);
  const ranges = [{ r0: 0, r1: 0, c0: 1, c1: 4 }];
  act(() => {
    grid.onSelection({ row: 0, column: 1, ranges });
  });
  act(() => {
    grid.onEdit?.({ row: 0, column: 1 }, 'z');
  });
  await user.keyboard('ed{Enter}');
  expect(editor.bulk.set).toHaveBeenCalledExactlyOnceWith(
    result,
    ranges,
    textValue('zed'),
  );
  expect(editor.stageCell).not.toHaveBeenCalled();
  const host = vi.mocked(mountGrid).mock.calls[0]?.[0];
  const paste = new Event('paste', { bubbles: true, cancelable: true });
  Object.defineProperty(paste, 'clipboardData', {
    value: { getData: () => '1\t2' },
  });
  host?.dispatchEvent(paste);
  expect(paste.defaultPrevented).toBe(true);
  expect(editor.bulk.paste).toHaveBeenCalledWith(result, ranges, '1\t2');
});

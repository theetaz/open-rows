// SPDX-License-Identifier: Apache-2.0
import { NULL_VALUE, textValue } from './cellValue';
import { useState } from 'react';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import type { GridContextTarget } from '@openrows/grid';
import { useTableContext } from './TableContextMenu';
import type { RowEditing } from './useRowEditing';
import { initialView } from './model';
import { emptyEdits } from './editHistory';
import { readRow } from './row';
vi.mock('./row', () => ({ readRow: vi.fn() }));
beforeEach(() => {
  vi.mocked(readRow).mockResolvedValue([]);
});
const result = {
  id: 'page',
  rows: 1,
  done: true,
  truncated: false,
  statement: 0,
  columns: [
    { name: 'id', sourceType: 'int4' },
    { name: 'note', sourceType: 'text' },
  ],
};
const draft = Promise.resolve({
  id: '{"id":"7"}',
  number: 1,
  kind: 'update' as const,
  original: { id: '7', note: 'n' },
  values: {},
});
function editing(readOnly = false): RowEditing {
  return {
    info: {
      key: ['id'],
      protected: ['id'],
      readOnly,
      columns: [
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
          name: 'note',
          dataType: 'text',
          nullable: true,
          default: null,
          comment: null,
          primaryKey: false,
          enumLabels: null,
        },
      ],
    },
    history: emptyEdits,
    busy: false,
    open: vi.fn(),
    edit: vi.fn(),
    quick: vi.fn(),
    insert: vi.fn(),
    draftFor: vi.fn(() => draft),
    stageCell: vi.fn(),
    fail: vi.fn(),
  } as unknown as RowEditing;
}
function Harness({
  editor,
  kind = 'cell',
}: {
  editor: RowEditing;
  kind?: GridContextTarget['kind'];
}): React.JSX.Element {
  const [view, setView] = useState(initialView);
  const context = useTableContext({ editing: editor, changeView: setView });
  return (
    <>
      <button
        type="button"
        onClick={() => {
          context.show(result, { row: 0, column: 1, kind, x: 25, y: 25 });
        }}
      >
        Open actions
      </button>
      {context.menu}
      <output aria-label="Applied filters">
        {JSON.stringify(view.applied)}
      </output>
      <output aria-label="Sort">{JSON.stringify(view.sort)}</output>
    </>
  );
}
it('opens inline value editing, stages NULL, EMPTY and pasted values, and restores focus on Escape', async () => {
  const user = userEvent.setup();
  const editor = editing();
  render(<Harness editor={editor} />);
  const opener = screen.getByRole('button', { name: 'Open actions' });
  await user.click(opener);
  expect(screen.getByRole('menuitem', { name: 'Edit value…' })).toHaveFocus();
  await user.keyboard('{Escape}');
  expect(screen.queryByRole('menu')).toBeNull();
  expect(opener).toHaveFocus();
  await user.click(opener);
  await user.click(screen.getByRole('menuitem', { name: 'Edit value…' }));
  expect(editor.edit).toHaveBeenCalledWith({ row: 0, column: 1 });
  expect(editor.open).not.toHaveBeenCalled();
  await user.click(opener);
  await user.click(screen.getByRole('menuitem', { name: 'Set NULL' }));
  expect(editor.draftFor).toHaveBeenCalledWith(result, 0);
  expect(editor.stageCell).toHaveBeenLastCalledWith(draft, 'note', NULL_VALUE);
  await user.click(opener);
  await user.click(screen.getByRole('menuitem', { name: 'Set EMPTY' }));
  expect(editor.stageCell).toHaveBeenLastCalledWith(
    draft,
    'note',
    textValue(''),
  );
  vi.spyOn(navigator.clipboard, 'readText').mockResolvedValue('pasted');
  await user.click(opener);
  await user.click(screen.getByRole('menuitem', { name: 'Paste value' }));
  await waitFor(() => {
    expect(editor.stageCell).toHaveBeenLastCalledWith(
      draft,
      'note',
      textValue('pasted'),
    );
  });
  expect(editor.quick).not.toHaveBeenCalled();
});
it.each(['null', 'empty'] as const)(
  'filters %s without treating NULL as an empty string',
  async (state) => {
    vi.mocked(readRow).mockResolvedValue([
      { name: 'id', type: 'int4', text: '7', state: 'normal' },
      {
        name: 'note',
        type: 'text',
        text: state === 'null' ? 'NULL' : 'EMPTY',
        state,
      },
    ]);
    const user = userEvent.setup();
    render(<Harness editor={editing()} />);
    await user.click(screen.getByRole('button', { name: 'Open actions' }));
    await waitFor(() => {
      expect(
        screen.getByRole('menuitem', { name: 'Filter by value' }),
      ).toBeEnabled();
    });
    await user.click(screen.getByRole('menuitem', { name: 'Filter by value' }));
    expect(screen.getByLabelText('Applied filters')).toHaveTextContent(
      state === 'null' ? 'isNull' : 'isEmpty',
    );
  },
);
it('keeps copy available while blocking every data mutation on a read-only connection', async () => {
  vi.mocked(readRow).mockResolvedValue([
    { name: 'id', type: 'int4', text: '7', state: 'normal' },
    { name: 'note', type: 'text', text: 'exact text', state: 'normal' },
  ]);
  const user = userEvent.setup();
  const write = vi.spyOn(navigator.clipboard, 'writeText').mockResolvedValue();
  render(<Harness editor={editing(true)} />);
  await user.click(screen.getByRole('button', { name: 'Open actions' }));
  for (const name of [
    'Edit value…',
    'Edit row…',
    'Set NULL',
    'Set EMPTY',
    'Paste value',
    'Add row',
    'Duplicate row',
    'Delete row',
  ])
    expect(screen.getByRole('menuitem', { name })).toBeDisabled();
  await waitFor(() => {
    expect(screen.getByRole('menuitem', { name: 'Copy value' })).toBeEnabled();
  });
  await user.click(screen.getByRole('menuitem', { name: 'Copy value' }));
  expect(write).toHaveBeenCalledWith('exact text');
  await waitFor(() => {
    expect(screen.queryByRole('menu')).toBeNull();
  });
});
it('right-click header actions set sorting without exposing row edits', async () => {
  const user = userEvent.setup();
  render(<Harness editor={editing()} kind="column" />);
  await user.click(screen.getByRole('button', { name: 'Open actions' }));
  expect(screen.queryByRole('menuitem', { name: 'Delete row' })).toBeNull();
  await user.click(screen.getByRole('menuitem', { name: 'Sort descending' }));
  expect(screen.getByLabelText('Sort')).toHaveTextContent('"descending":true');
});

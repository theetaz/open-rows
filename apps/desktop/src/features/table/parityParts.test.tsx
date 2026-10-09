// SPDX-License-Identifier: Apache-2.0
import { useRef, useState } from 'react';
import { render, renderHook, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { BulkDetails } from './BulkDetails';
import { NULL_VALUE, textValue } from './cellValue';
import { emptyHistory } from './editHistory';
import type { PositionMemory } from './gridPosition';
import type { KeyJumps } from './links/useKeyJumps';
import { initialView } from './model';
import { useTableContext } from './TableContextMenu';
import { useDiscardReload } from './useDiscardReload';
import type { RowEditing } from './useRowEditing';

vi.mock('./row', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./row')>()),
  readRow: vi.fn(() => Promise.resolve([])),
}));

const result = {
  id: 'page',
  rows: 3,
  done: true,
  truncated: false,
  statement: 0,
  columns: [{ name: 'id', sourceType: 'int4' }],
};

function editing(staged: number): RowEditing {
  return {
    info: null,
    history: {
      ...emptyHistory,
      current: Array.from({ length: staged }, (_, index) => ({ id: index })),
    },
    busy: false,
    discard: vi.fn(),
    fail: vi.fn(),
  } as unknown as RowEditing;
}

it('stages a typed value or NULL in every selected row from the bulk inspector', async () => {
  const user = userEvent.setup();
  const stage = vi.fn();
  render(
    <BulkDetails
      table="users"
      search=""
      bulk={{
        rows: 3,
        stage,
        fields: [
          { name: 'id', type: 'int4', changed: false },
          { name: 'status', type: 'text', common: 'open', changed: true },
          { name: 'note', type: 'text', common: null, changed: false },
        ],
      }}
    />,
  );
  expect(screen.getByText(/3 rows selected/)).toBeVisible();
  expect(screen.getByLabelText('id')).toHaveAttribute(
    'placeholder',
    'Multiple values',
  );
  expect(screen.getByLabelText('note')).toHaveAttribute('placeholder', 'NULL');
  const status = screen.getByLabelText('status');
  expect(status).toHaveValue('open');
  await user.clear(status);
  await user.type(status, 'closed{Enter}');
  expect(stage).toHaveBeenCalledWith('status', textValue('closed'));
  await user.click(
    screen.getByRole('button', {
      name: 'Set id to NULL in every selected row',
    }),
  );
  expect(stage).toHaveBeenLastCalledWith('id', NULL_VALUE);
});

it('shows the bulk fields read-only without an editable key', () => {
  render(
    <BulkDetails
      table="users"
      search="st"
      bulk={{
        rows: 2,
        fields: [
          { name: 'id', type: 'int4', changed: false },
          { name: 'status', type: 'text', common: 'open', changed: false },
        ],
      }}
    />,
  );
  expect(screen.queryByLabelText('id')).toBeNull();
  expect(screen.getByLabelText('status')).toHaveAttribute('readonly');
  expect(screen.getByText(/read-only/)).toBeVisible();
});

function useFlow(staged: number, reload: () => void) {
  const position = useRef<PositionMemory>({
    position: { x: 0, y: 90, row: 4, column: 1 },
    restore: false,
  });
  const [editor] = useState(() => editing(staged));
  return {
    flow: useDiscardReload({ editing: editor, reload, position }),
    editor,
    position,
  };
}

it('confirms Discard Changes and reloads, returning to the grid position', async () => {
  const user = userEvent.setup();
  const reload = vi.fn();
  const { result: hook, rerender } = renderHook(() => useFlow(2, reload));
  hook.current.flow.discard();
  rerender();
  const sheet = render(hook.current.flow.sheet ?? <></>);
  expect(
    screen.getByRole('dialog', { name: 'Discard 2 staged changes?' }),
  ).toBeVisible();
  expect(screen.getByText(/nothing is rolled back/)).toBeVisible();
  await user.click(screen.getByRole('button', { name: 'Discard changes' }));
  expect(hook.current.editor.discard).toHaveBeenCalledOnce();
  expect(reload).toHaveBeenCalledOnce();
  expect(hook.current.position.current.restore).toBe(true);
  sheet.unmount();
});

it('asks before reloading with staged edits and can keep them', async () => {
  const user = userEvent.setup();
  const reload = vi.fn();
  const { result: hook, rerender } = renderHook(() => useFlow(1, reload));
  hook.current.flow.reload();
  rerender();
  render(hook.current.flow.sheet ?? <></>);
  await user.click(
    screen.getByRole('button', { name: 'Reload, keep changes' }),
  );
  expect(reload).toHaveBeenCalledOnce();
  expect(hook.current.editor.discard).not.toHaveBeenCalled();
  const clean = renderHook(() => useFlow(0, reload));
  clean.result.current.flow.reload();
  clean.result.current.flow.discard();
  expect(reload).toHaveBeenCalledTimes(2);
  expect(clean.result.current.flow.sheet).toBeNull();
});

it('lists referenced and referencing tables in the row menu', async () => {
  const user = userEvent.setup();
  const open = vi.fn();
  const outgoing = {
    name: 'users_org',
    schema: 'app',
    table: 'orgs',
    columns: ['org_id'],
    targetColumns: ['id'],
  };
  const incoming = {
    name: 'notes_user',
    schema: 'app',
    table: 'notes',
    columns: ['id'],
    targetColumns: ['user_id'],
  };
  const jumps = {
    links: { outgoing: [outgoing], incoming: [incoming] },
    columns: new Set(['org_id']),
    open,
    openCell: vi.fn(),
  } satisfies KeyJumps;
  function Menu(): React.JSX.Element {
    const context = useTableContext({
      editing: editing(0),
      changeView: () => initialView,
      jumps,
    });
    return (
      <>
        <button
          type="button"
          onClick={() => {
            context.show(result, {
              row: 1,
              column: 0,
              kind: 'row',
              x: 10,
              y: 10,
              ranges: [],
            });
          }}
        >
          Open actions
        </button>
        {context.menu}
      </>
    );
  }
  render(<Menu />);
  await user.click(screen.getByRole('button', { name: 'Open actions' }));
  await user.click(screen.getByRole('menuitem', { name: /Referenced by/ }));
  await user.click(
    await screen.findByRole('menuitem', { name: 'notes (user_id)' }),
  );
  expect(open).toHaveBeenCalledWith(result, 1, incoming);
  await user.click(screen.getByRole('button', { name: 'Open actions' }));
  await user.click(
    screen.getByRole('menuitem', { name: /Open referenced row/ }),
  );
  await user.click(await screen.findByRole('menuitem', { name: 'orgs (id)' }));
  expect(open).toHaveBeenLastCalledWith(result, 1, outgoing);
});

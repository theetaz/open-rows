// SPDX-License-Identifier: Apache-2.0
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import type { TableColumnDto } from '@openrows/ipc';
import { ThemeProvider } from '@openrows/ui';
import { RowDetails } from './RowDetails';
import type { RowEditor, RowField, SelectedRow } from './row';

const column = (
  name: string,
  dataType: string,
  facts: Partial<TableColumnDto> = {},
): TableColumnDto => ({
  name,
  dataType,
  nullable: true,
  default: null,
  comment: null,
  primaryKey: false,
  enumLabels: null,
  ...facts,
});
const columns = [
  column('id', 'integer', { nullable: false, primaryKey: true }),
  column('name', 'character varying(80)', { nullable: false }),
  column('note', 'text'),
  column('doc', 'jsonb'),
  column('bad', 'json'),
  column('total', 'numeric', { nullable: false, default: '0' }),
  column('blob', 'bytea'),
  column('computed', 'integer'),
];
const text = (name: string, value: string, type = 'text'): RowField => ({
  index: 0,
  name,
  type,
  text: value,
  state: 'normal',
  value: { kind: 'text', value },
});
const fields: RowField[] = [
  text('id', '7', 'int4'),
  text('name', 'Ada'),
  {
    index: 2,
    name: 'note',
    type: 'text',
    text: 'NULL',
    state: 'null',
    value: { kind: 'null' },
  },
  text('doc', '{"b":[1,12345678901234567890],"a":{}}', 'jsonb'),
  text('bad', '{oops', 'json'),
  text('total', '1.50', 'numeric'),
  text('blob', '\\x00ff', 'bytea'),
  text('computed', '14', 'int4'),
];

function editor(overrides: Partial<RowEditor> = {}): RowEditor {
  return {
    columns,
    protected: ['computed'],
    ready: true,
    load: vi.fn(),
    stage: vi.fn(),
    addFile: vi.fn(),
    ...overrides,
  };
}

function show(row: Partial<SelectedRow> = {}) {
  const user = userEvent.setup();
  const selected: SelectedRow = {
    table: 'users',
    number: 7,
    fields,
    editor: editor(),
    ...row,
  };
  render(
    <ThemeProvider>
      <RowDetails row={selected} />
    </ThemeProvider>,
  );
  return { user, editor: selected.editor };
}

const box = (name: string): HTMLElement =>
  screen.getByLabelText(`Value of ${name}`);

async function menu(
  user: ReturnType<typeof userEvent.setup>,
  name: string,
): Promise<HTMLElement> {
  await user.click(
    screen.getByRole('button', { name: `Value options for ${name}` }),
  );
  return screen.getByRole('menu', { name: `Value of ${name}` });
}

it('lays out name, muted type and an editable monospace value per field', () => {
  show();
  const list = screen.getByRole('list', { name: 'Row fields' });
  const name = within(list).getAllByRole('listitem')[1];
  expect(name).toHaveTextContent('name');
  expect(name).toHaveTextContent('text');
  expect(box('name')).toHaveValue('Ada');
  expect(box('name')).not.toHaveAttribute('readonly');
  expect(box('note')).toHaveValue('');
  expect(box('note')).toHaveAttribute('placeholder', 'NULL');
  expect(box('computed')).toHaveAttribute('readonly');
});

it('stages typed text on blur or Enter and reverts on Escape', async () => {
  const { user, editor: rowEditor } = show();
  await user.clear(box('name'));
  await user.type(box('name'), 'Grace');
  await user.tab();
  expect(rowEditor?.stage).toHaveBeenLastCalledWith('name', {
    kind: 'text',
    value: 'Grace',
  });
  await user.click(box('note'));
  await user.type(box('note'), 'hello{Enter}');
  expect(rowEditor?.stage).toHaveBeenLastCalledWith('note', {
    kind: 'text',
    value: 'hello',
  });
  vi.mocked(rowEditor?.stage ?? vi.fn()).mockClear();
  await user.click(box('total'));
  await user.type(box('total'), '99{Escape}');
  expect(box('total')).toHaveValue('1.50');
  await user.tab();
  expect(rowEditor?.stage).not.toHaveBeenCalled();
});

it('needs ⌘Enter to stage once the value spans several lines', async () => {
  const { user, editor: rowEditor } = show();
  await user.clear(box('note'));
  await user.type(box('note'), 'one{Shift>}{Enter}{/Shift}two{Enter}');
  expect(rowEditor?.stage).not.toHaveBeenCalled();
  expect(box('note')).toHaveValue('one\ntwo\n');
  await user.keyboard('{Meta>}{Enter}{/Meta}');
  expect(rowEditor?.stage).toHaveBeenCalledWith('note', {
    kind: 'text',
    value: 'one\ntwo\n',
  });
});

it('stays read-only until the exact snapshot loads, loading it on focus', async () => {
  const load = vi.fn();
  const { user } = show({
    fields: [
      { index: 0, name: 'id', type: 'int4', text: '7', state: 'normal' },
    ],
    editor: editor({ ready: false, load }),
  });
  expect(box('id')).toHaveAttribute('readonly');
  await user.click(box('id'));
  expect(load).toHaveBeenCalled();
});

it('stages NULL, DEFAULT and EMPTY from the value menu', async () => {
  const { user, editor: rowEditor } = show();
  await user.click(
    within(await menu(user, 'note')).getByRole('menuitem', { name: 'NULL' }),
  );
  expect(rowEditor?.stage).toHaveBeenLastCalledWith('note', { kind: 'null' });
  await user.click(
    within(await menu(user, 'note')).getByRole('menuitem', {
      name: 'DEFAULT',
    }),
  );
  expect(rowEditor?.stage).toHaveBeenLastCalledWith('note', {
    kind: 'default',
  });
  await user.click(
    within(await menu(user, 'name')).getByRole('menuitem', { name: 'EMPTY' }),
  );
  expect(rowEditor?.stage).toHaveBeenLastCalledWith('name', {
    kind: 'text',
    value: '',
  });
});

it('disables NULL for NOT NULL, DEFAULT without one, EMPTY for non-text', async () => {
  const { user } = show();
  const name = within(await menu(user, 'name'));
  expect(name.getByRole('menuitem', { name: 'NULL' })).toBeDisabled();
  expect(name.getByRole('menuitem', { name: 'DEFAULT' })).toBeDisabled();
  expect(name.getByRole('menuitem', { name: 'EMPTY' })).toBeEnabled();
  expect(name.getByRole('menuitem', { name: 'PRETTY' })).toBeDisabled();
  await user.keyboard('{Escape}');
  const total = within(await menu(user, 'total'));
  expect(total.getByRole('menuitem', { name: 'NULL' })).toBeDisabled();
  expect(total.getByRole('menuitem', { name: 'DEFAULT' })).toBeEnabled();
  expect(total.getByRole('menuitem', { name: 'EMPTY' })).toBeDisabled();
  expect(total.getByRole('menuitem', { name: 'Add a file…' })).toBeDisabled();
  await user.keyboard('{Escape}');
  const computed = within(await menu(user, 'computed'));
  for (const item of computed.getAllByRole('menuitem'))
    expect(item).toBeDisabled();
});

it('pretty-prints and minifies valid JSON exactly, never invalid JSON', async () => {
  const { user, editor: rowEditor } = show();
  await user.click(
    within(await menu(user, 'doc')).getByRole('menuitem', { name: 'PRETTY' }),
  );
  expect(rowEditor?.stage).toHaveBeenLastCalledWith('doc', {
    kind: 'text',
    value: '{\n  "b": [\n    1,\n    12345678901234567890\n  ],\n  "a": {}\n}',
  });
  await user.click(
    within(await menu(user, 'doc')).getByRole('menuitem', { name: 'MINIFY' }),
  );
  expect(rowEditor?.stage).toHaveBeenLastCalledWith('doc', {
    kind: 'text',
    value: '{"b":[1,12345678901234567890],"a":{}}',
  });
  const bad = within(await menu(user, 'bad'));
  expect(bad.getByRole('menuitem', { name: 'PRETTY' })).toBeDisabled();
  expect(bad.getByRole('menuitem', { name: 'MINIFY' })).toBeDisabled();
});

it('adds a file through the native picker for bytea and text fields', async () => {
  const { user, editor: rowEditor } = show();
  await user.click(
    within(await menu(user, 'blob')).getByRole('menuitem', {
      name: 'Add a file…',
    }),
  );
  expect(rowEditor?.addFile).toHaveBeenCalledWith('blob');
});

it('marks staged and deleted fields and hides editing without an editor', () => {
  const { unmount } = render(
    <RowDetails
      row={{
        table: 'users',
        number: 1,
        fields: [
          {
            index: 2,
            name: 'note',
            type: 'text',
            text: 'DEFAULT',
            state: 'changed',
            value: { kind: 'default' },
          },
        ],
        editor: editor(),
      }}
    />,
  );
  expect(box('note')).toHaveAttribute('placeholder', 'DEFAULT');
  expect(box('note').closest('[data-state]')).toHaveAttribute(
    'data-state',
    'changed',
  );
  unmount();
  render(<RowDetails row={{ table: 'users', number: 1, fields }} />);
  expect(box('name')).toHaveAttribute('readonly');
  expect(
    screen.queryByRole('button', { name: /Value options/ }),
  ).not.toBeInTheDocument();
});

it('shows booleans as TRUE and FALSE but stages the exact stored text', async () => {
  const { user, editor: rowEditor } = show({
    fields: [text('flag', 't', 'bool'), text('off', 'f', 'bool')],
    editor: editor({
      columns: [...columns, column('flag', 'boolean'), column('off', 'bool')],
    }),
  });
  expect(box('flag')).toHaveValue('TRUE');
  expect(box('off')).toHaveValue('FALSE');
  await user.clear(box('flag'));
  await user.type(box('flag'), 'f');
  await user.tab();
  expect(rowEditor?.stage).toHaveBeenLastCalledWith('flag', {
    kind: 'text',
    value: 'f',
  });
});

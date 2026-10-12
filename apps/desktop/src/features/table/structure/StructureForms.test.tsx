// SPDX-License-Identifier: Apache-2.0
import {
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
// Load the lazy table module before interactions so transform time is not UI latency.
import '../TableTab';
import { ThemeProvider } from '@openrows/ui';
import { tableAlterPreview } from '@openrows/ipc';
import type { SchemaChangeDto } from '@openrows/ipc';
import type { Connections } from '../../connections';
import { profile } from '../../../test/ipcStub';
import { QueryContent, useQueryTabs } from '../../query';
import { openTableView } from '../open';

vi.mock('@openrows/ipc', async () =>
  (await import('../../../test/ipcStub')).ipcStub(),
);
vi.mock('./SqlViewer', () => ({
  SqlViewer: (props: { sql: string; label: string }) => (
    <pre aria-label={props.label}>{props.sql}</pre>
  ),
}));

const connection = profile('orders-local');
const connections = {
  active: connection,
  statusOf: () => ({
    state: 'connected',
    insecure: [],
    databases: [{ name: 'orders', state: 'open', sessionId: 'session-1' }],
  }),
  connect: vi.fn(),
} as unknown as Connections;
const trigger = vi.fn();

beforeEach(() => {
  vi.mocked(tableAlterPreview).mockClear();
  trigger.mockReset();
});

function Harness(): React.JSX.Element {
  const tabs = useQueryTabs();
  const scope = { connectionId: connection.id, database: 'orders' };
  return (
    <>
      <button
        type="button"
        onClick={() => {
          openTableView(tabs, scope, {
            database: 'orders',
            schema: 'public',
            name: 'orders',
            kind: 'table',
          });
        }}
      >
        Open orders
      </button>
      <QueryContent
        connections={connections}
        tabs={tabs}
        database="orders"
        onCell={vi.fn()}
        onRow={vi.fn()}
        onTrigger={trigger}
      />
    </>
  );
}

async function structure(): Promise<ReturnType<typeof userEvent.setup>> {
  const user = userEvent.setup();
  render(
    <ThemeProvider>
      <Harness />
    </ThemeProvider>,
  );
  await user.click(screen.getByRole('button', { name: 'Open orders' }));
  await screen.findByRole('region', { name: 'orders table' });
  await user.click(screen.getByRole('radio', { name: 'Structure' }));
  await screen.findByRole('grid', { name: 'Columns' });
  return user;
}

/** The changes of the last live preview. */
const previewed = (): readonly SchemaChangeDto[] =>
  vi.mocked(tableAlterPreview).mock.lastCall?.[1] ?? [];

function rowOf(name: string): HTMLElement {
  const found = within(screen.getByRole('grid', { name: 'Columns' }))
    .getAllByRole('row')
    .find((item) => within(item).queryByText(name));
  if (!found) throw new Error(`no row ${name}`);
  return found;
}

it('edits a column in a form with type size, nullability and collation, previewing the DDL', async () => {
  const user = await structure();
  fireEvent.contextMenu(within(rowOf('customer_id')).getByText('uuid'));
  await user.click(screen.getByRole('menuitem', { name: 'Edit column…' }));
  const sheet = screen.getByRole('dialog', { name: 'Edit column customer_id' });
  const type = within(sheet).getByRole('combobox', { name: 'Type' });
  await user.clear(type);
  await user.type(type, 'varchar');
  await user.type(within(sheet).getByRole('textbox', { name: 'Length' }), '36');
  await user.click(within(sheet).getByRole('switch', { name: 'Allow NULL' }));
  await user.type(
    within(sheet).getByRole('combobox', { name: 'Collation' }),
    'C',
  );
  await waitFor(() => {
    expect(previewed()).toEqual([
      { kind: 'columnType', column: 'customer_id', dataType: 'varchar(36)' },
      {
        kind: 'columnCollation',
        column: 'customer_id',
        charset: null,
        collation: 'C',
      },
      { kind: 'columnNullable', column: 'customer_id', nullable: false },
    ]);
  });
  expect(within(sheet).getByLabelText('Statements to run')).toHaveTextContent(
    'varchar(36)',
  );
  await user.click(
    within(sheet).getByRole('button', { name: 'Stage changes' }),
  );
  expect(screen.getByText('3 staged structure changes')).toBeVisible();
  expect(rowOf('customer_id')).toHaveTextContent('varchar(36)');
});

it('adds a column through the form and shows the staged SQL in the bar', async () => {
  const user = await structure();
  fireEvent.contextMenu(within(rowOf('name')).getByText('text'));
  await user.click(screen.getByRole('menuitem', { name: 'Add column…' }));
  const sheet = screen.getByRole('dialog', { name: 'Add column' });
  const name = within(sheet).getByRole('textbox', { name: 'Name' });
  await user.clear(name);
  await user.type(name, 'total');
  const type = within(sheet).getByRole('combobox', { name: 'Type' });
  await user.clear(type);
  await user.type(type, 'numeric');
  await user.type(
    within(sheet).getByRole('textbox', { name: 'Precision' }),
    '10',
  );
  await user.type(within(sheet).getByRole('textbox', { name: 'Scale' }), '2');
  await user.selectOptions(
    within(sheet).getByRole('combobox', { name: 'Default' }),
    'expression',
  );
  await user.type(
    within(sheet).getByRole('textbox', { name: 'Default expression' }),
    '0',
  );
  await user.click(within(sheet).getByRole('button', { name: 'Stage column' }));
  await user.click(
    screen.getByRole('button', { name: /1 staged structure change/ }),
  );
  await waitFor(() => {
    expect(previewed()).toEqual([
      {
        kind: 'addColumn',
        name: 'total',
        dataType: 'numeric(10,2)',
        nullable: true,
        default: { kind: 'expression', sql: '0' },
        comment: null,
        identity: 'none',
        collation: null,
        charset: null,
      },
    ]);
  });
  expect(await screen.findByLabelText('Statements to run')).toHaveTextContent(
    'numeric(10,2)',
  );
});

it('lists constraints, stages a drop and adds a check and a foreign key', async () => {
  const user = await structure();
  await user.click(screen.getByRole('button', { name: 'Constraints' }));
  let sheet = screen.getByRole('dialog', { name: 'Constraints' });
  expect(within(sheet).getByText(/orders_customer_id_fkey/)).toBeVisible();
  const fk = within(sheet).getAllByRole('button', { name: 'Delete' })[1];
  if (!fk) throw new Error('no delete');
  await user.click(fk);
  expect(within(sheet).getByRole('button', { name: 'Restore' })).toBeVisible();
  await user.click(within(sheet).getByRole('button', { name: 'Add check…' }));
  sheet = screen.getByRole('dialog', { name: 'Add check' });
  await user.type(
    within(sheet).getByRole('textbox', { name: 'Condition' }),
    'id > 0',
  );
  await user.click(within(sheet).getByRole('button', { name: 'Stage check' }));
  await user.click(screen.getByRole('button', { name: 'Constraints' }));
  await user.click(screen.getByRole('button', { name: 'Add foreign key…' }));
  sheet = screen.getByRole('dialog', { name: 'Add foreign key' });
  const group = (name: string): HTMLElement =>
    within(sheet).getByRole('group', { name });
  await user.click(
    within(group('Columns')).getByRole('checkbox', { name: 'name' }),
  );
  await user.type(
    within(sheet).getByRole('combobox', { name: 'Referenced table' }),
    'customers',
  );
  await waitFor(() => {
    expect(
      within(group('Referenced columns')).getByRole('checkbox', {
        name: /^id/,
      }),
    ).toBeChecked();
  });
  await user.selectOptions(
    within(sheet).getByRole('combobox', { name: 'On delete' }),
    'cascade',
  );
  await user.click(
    within(sheet).getByRole('button', { name: 'Stage foreign key' }),
  );
  expect(screen.getByText('3 staged structure changes')).toBeVisible();
  await user.click(
    screen.getByRole('button', { name: /3 staged structure changes/ }),
  );
  await waitFor(() => {
    expect(previewed()).toEqual([
      { kind: 'dropConstraint', name: 'orders_customer_id_fkey' },
      { kind: 'addCheck', check: { name: null, expression: 'id > 0' } },
      {
        kind: 'addForeignKey',
        key: {
          name: null,
          columns: ['name'],
          target: { schema: 'public', table: 'customers', columns: ['id'] },
          onDelete: 'cascade',
          onUpdate: 'noAction',
        },
      },
    ]);
  });
});

it('creates indexes with a sort order and a partial predicate', async () => {
  const user = await structure();
  await user.click(screen.getByRole('radio', { name: 'Index' }));
  await user.click(screen.getByRole('button', { name: 'Add index' }));
  const sheet = screen.getByRole('dialog', { name: 'Add index' });
  await user.click(within(sheet).getByRole('checkbox', { name: 'name' }));
  await user.click(within(sheet).getByRole('checkbox', { name: 'id' }));
  await user.selectOptions(
    within(sheet).getByRole('combobox', { name: 'Order of name' }),
    'desc',
  );
  await user.selectOptions(
    within(sheet).getByRole('combobox', { name: 'Algorithm' }),
    'brin',
  );
  await user.type(
    within(sheet).getByRole('textbox', { name: 'Condition (partial index)' }),
    'id > 10',
  );
  await waitFor(() => {
    expect(previewed()).toEqual([
      {
        kind: 'createIndex',
        name: null,
        columns: ['name', 'id'],
        unique: false,
        method: 'brin',
        descending: ['name'],
        predicate: 'id > 10',
      },
    ]);
  });
  await user.click(within(sheet).getByRole('button', { name: 'Stage index' }));
  expect(screen.getByText('1 staged structure change')).toBeVisible();
});

it('opens the trigger editors from the triggers sheet', async () => {
  const user = await structure();
  await user.click(screen.getByRole('button', { name: 'Triggers' }));
  const sheet = screen.getByRole('dialog', { name: 'Triggers on orders' });
  await user.click(within(sheet).getByRole('button', { name: 'Edit…' }));
  expect(trigger).toHaveBeenLastCalledWith(
    { database: 'orders', schema: 'public', name: 'orders' },
    'orders_touch',
    false,
  );
  await user.click(screen.getByRole('button', { name: 'Triggers' }));
  await user.click(screen.getByRole('button', { name: 'New trigger…' }));
  expect(trigger).toHaveBeenLastCalledWith(
    { database: 'orders', schema: 'public', name: 'orders' },
    null,
    false,
  );
});

it('previews and stages ordered unique and primary key constraints', async () => {
  const user = await structure();
  await user.click(screen.getByRole('button', { name: 'Constraints' }));
  await user.click(screen.getByRole('button', { name: 'Add key…' }));
  const sheet = screen.getByRole('dialog', { name: 'Add key constraint' });
  await user.type(
    within(sheet).getByRole('textbox', { name: 'Name' }),
    'name_key',
  );
  await user.click(within(sheet).getByRole('checkbox', { name: 'name' }));
  await user.click(
    within(sheet).getByRole('checkbox', { name: 'customer_id' }),
  );
  await waitFor(() => {
    expect(previewed()).toContainEqual({
      kind: 'addUnique',
      key: { name: 'name_key', columns: ['name', 'customer_id'] },
    });
  });
  await user.selectOptions(
    within(sheet).getByRole('combobox', { name: 'Constraint' }),
    'addPrimaryKey',
  );
  await waitFor(() => {
    expect(previewed()).toContainEqual({
      kind: 'addPrimaryKey',
      key: { name: 'name_key', columns: ['name', 'customer_id'] },
    });
  });
  await user.click(within(sheet).getByRole('button', { name: 'Stage key' }));
  expect(screen.getByText('1 staged structure change')).toBeVisible();
});

it('previews explicit PostgreSQL conversion and explains unavailable reordering', async () => {
  const user = await structure();
  fireEvent.contextMenu(within(rowOf('name')).getByText('text'));
  await user.click(screen.getByRole('menuitem', { name: 'Edit column…' }));
  const sheet = screen.getByRole('dialog', { name: 'Edit column name' });
  expect(
    within(sheet).queryByRole('combobox', { name: 'Position' }),
  ).not.toBeInTheDocument();
  expect(
    within(sheet).getByText(/Column reordering is unavailable/),
  ).toBeVisible();
  const type = within(sheet).getByRole('combobox', { name: 'Type' });
  await user.clear(type);
  await user.type(type, 'integer');
  await user.type(
    within(sheet).getByRole('textbox', { name: 'USING conversion expression' }),
    'length(name)',
  );
  await waitFor(() => {
    expect(previewed()).toEqual([
      {
        kind: 'columnTypeUsing',
        column: 'name',
        dataType: 'integer',
        expression: 'length(name)',
      },
    ]);
  });
});

it('previews a concurrent index as a separate nontransactional change', async () => {
  const user = await structure();
  await user.click(screen.getByRole('radio', { name: 'Index' }));
  await user.click(screen.getByRole('button', { name: /Add index/ }));
  const sheet = screen.getByRole('dialog', { name: 'Add index' });
  await user.click(within(sheet).getByRole('checkbox', { name: 'name' }));
  await user.click(
    within(sheet).getByRole('checkbox', { name: 'Build concurrently' }),
  );
  await waitFor(() => {
    expect(previewed()).toContainEqual({
      kind: 'createIndexConcurrently',
      index: {
        name: null,
        columns: ['name'],
        unique: false,
        method: 'btree',
        descending: [],
        predicate: null,
      },
    });
  });
  expect(within(sheet).getByText(/outside a transaction/)).toBeVisible();
});

it('removes an earlier column move when Keep position is selected', async () => {
  connection.objects.columnReordering = true;
  try {
    const user = await structure();
    const edit = async (): Promise<HTMLElement> => {
      fireEvent.contextMenu(within(rowOf('name')).getByText('text'));
      await user.click(screen.getByRole('menuitem', { name: 'Edit column…' }));
      return screen.getByRole('dialog', { name: 'Edit column name' });
    };
    let sheet = await edit();
    await user.selectOptions(
      within(sheet).getByRole('combobox', { name: 'Position' }),
      'first',
    );
    await user.click(
      within(sheet).getByRole('button', { name: 'Stage changes' }),
    );
    expect(screen.getByText('1 staged structure change')).toBeVisible();
    sheet = await edit();
    await user.selectOptions(
      within(sheet).getByRole('combobox', { name: 'Position' }),
      '',
    );
    expect(within(sheet).getByText('Nothing to change yet.')).toBeVisible();
    await user.click(
      within(sheet).getByRole('button', { name: 'Stage changes' }),
    );
    expect(
      screen.queryByText('1 staged structure change'),
    ).not.toBeInTheDocument();
  } finally {
    connection.objects.columnReordering = false;
  }
});

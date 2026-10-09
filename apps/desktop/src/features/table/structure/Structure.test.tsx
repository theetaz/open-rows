// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import {
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import { tableAlter, tableDetail } from '@openrows/ipc';
import type { Connections } from '../../connections';
import { profile } from '../../../test/ipcStub';
import { sampleDetail } from '../../../test/structureStub';
import { QueryContent, useQueryTabs } from '../../query';
import { openTableView } from '../open';
import { TableInfoPanel } from './TableInfoPanel';
import type { SelectedTable } from './TableInfoPanel';

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
const opened = vi.fn();

beforeEach(() => {
  opened.mockReset();
  vi.mocked(tableAlter).mockClear();
  vi.mocked(tableDetail).mockClear();
});

function Harness(): React.JSX.Element {
  const tabs = useQueryTabs();
  const [table, setTable] = useState<SelectedTable | null>(null);
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
        onTableInfo={setTable}
        onOpenTable={opened}
      />
      {table && <TableInfoPanel table={table} />}
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

const columns = (): HTMLElement =>
  screen.getByRole('grid', { name: 'Columns' });
const row = (name: string): HTMLElement => {
  const found = within(columns())
    .getAllByRole('row')
    .find((item) => within(item).queryByText(name));
  if (!found) throw new Error(`no row ${name}`);
  return found;
};

it('shows the column grid, primary key, table info and footer tools', async () => {
  await structure();
  const grid = columns();
  expect(grid).toHaveTextContent('GENERATED ALWAYS AS IDENTITY');
  expect(grid).toHaveTextContent("'anon'::text");
  expect(grid).toHaveTextContent('customers(id)');
  expect(within(row('customer_id')).getAllByText('NULL')[0]).toHaveAttribute(
    'data-muted',
    'true',
  );
  expect(screen.getByLabelText('Primary key')).toHaveTextContent('id');
  expect(screen.getByRole('button', { name: 'Add column' })).toBeEnabled();
  expect(screen.getByRole('button', { name: 'Triggers' })).toBeVisible();
  const info = screen.getByLabelText('orders table info');
  expect(info).toHaveTextContent('81 KB');
  expect(info).toHaveTextContent('16 KB');
  expect(info).toHaveTextContent('1,788');
});

it('stages inline edits, pickers, deletes and new columns, then reviews and commits', async () => {
  const user = await structure();
  await user.dblClick(within(row('name')).getByText('name'));
  const input = screen.getByRole('textbox', {
    name: 'Edit column_name of name',
  });
  await user.clear(input);
  await user.type(input, 'full_name{Enter}');
  await user.click(
    screen.getByRole('button', { name: 'Choose nullable of full_name' }),
  );
  await user.click(screen.getByRole('menuitemradio', { name: /NO/ }));
  fireEvent.contextMenu(within(row('customer_id')).getByText('uuid'));
  await user.click(screen.getByRole('menuitem', { name: 'Delete column' }));
  await user.click(screen.getByRole('button', { name: 'Add column' }));
  const added = screen.getByRole('textbox', {
    name: 'Edit column_name of column_4',
  });
  await user.clear(added);
  await user.type(added, 'notes{Enter}');
  expect(screen.getByText('4 staged structure changes')).toBeVisible();
  vi.mocked(tableAlter).mockResolvedValueOnce({
    kind: 'prepared',
    token: 'review-1',
    statements: ['ALTER TABLE "public"."orders" DROP COLUMN "customer_id"'],
    warnings: ['Dropping column "customer_id" permanently deletes its data.'],
    destructive: true,
    confirmName: 'orders',
  });
  await user.click(screen.getByRole('button', { name: 'Review & commit' }));
  expect(vi.mocked(tableAlter).mock.lastCall?.[0]).toEqual({
    sessionId: 'session-1',
    target: {
      profileId: connection.id,
      database: 'orders',
      schema: 'public',
      table: 'orders',
    },
    changes: [
      { kind: 'renameColumn', column: 'name', name: 'full_name' },
      { kind: 'columnNullable', column: 'name', nullable: false },
      { kind: 'dropColumn', column: 'customer_id' },
      {
        kind: 'addColumn',
        name: 'notes',
        dataType: 'text',
        nullable: true,
        default: { kind: 'none' },
        comment: null,
      },
    ],
    confirmToken: null,
    confirmName: null,
  });
  const sheet = await screen.findByRole('dialog', {
    name: 'Review structure changes',
  });
  expect(within(sheet).getByLabelText('SQL to commit')).toHaveTextContent(
    'DROP COLUMN "customer_id";',
  );
  expect(within(sheet).getByLabelText('Warnings')).toHaveTextContent(
    'permanently deletes',
  );
  const commit = within(sheet).getByRole('button', { name: 'Commit changes' });
  expect(commit).toBeDisabled();
  await user.type(within(sheet).getByRole('textbox'), 'orders');
  vi.mocked(tableAlter).mockResolvedValueOnce({ kind: 'committed' });
  await user.click(commit);
  expect(vi.mocked(tableAlter).mock.lastCall?.[0]).toMatchObject({
    confirmToken: 'review-1',
    confirmName: 'orders',
  });
  await waitFor(() => {
    expect(tableDetail).toHaveBeenCalledTimes(2);
  });
  expect(screen.queryByText(/staged structure change/)).toBeNull();
});

it('navigates with the keyboard and edits defaults as expressions or literals', async () => {
  const user = await structure();
  const grid = columns();
  grid.focus();
  const active = (): string | null =>
    document.getElementById(grid.getAttribute('aria-activedescendant') ?? '')
      ?.textContent ?? null;
  expect(active()).toBe('id');
  await user.keyboard('{ArrowDown}{ArrowRight}');
  expect(active()).toContain('text');
  await user.keyboard('{ArrowRight}{ArrowRight}{ArrowRight}{Enter}');
  const input = screen.getByRole('textbox', {
    name: 'Edit column_default of name',
  });
  await user.clear(input);
  await user.type(input, 'now(){Enter}');
  expect(grid).toHaveTextContent('now()');
  await user.click(
    screen.getByRole('button', { name: 'Choose default of name' }),
  );
  await user.click(screen.getByRole('menuitem', { name: 'Text literal…' }));
  const literal = await screen.findByRole('textbox', {
    name: 'Edit column_default of name',
  });
  await user.type(literal, "it's{Enter}");
  expect(grid).toHaveTextContent("'it''s'");
  await user.keyboard('{Escape}');
  expect(screen.getByText('1 staged structure change')).toBeVisible();
});

it('opens a foreign key target, the DDL popover and the triggers sheet', async () => {
  const user = await structure();
  await user.click(
    screen.getByRole('button', { name: 'Open public.customers' }),
  );
  expect(opened).toHaveBeenCalledWith({
    database: 'orders',
    schema: 'public',
    name: 'customers',
  });
  await user.click(screen.getByRole('button', { name: 'DDL' }));
  const ddl = screen.getByRole('dialog', { name: 'Table definition' });
  expect(ddl).toHaveTextContent('CREATE TABLE "public"."orders"');
  expect(ddl).toHaveTextContent('Estimated rows 1,788');
  await user.keyboard('{Escape}');
  expect(screen.queryByRole('dialog', { name: 'Table definition' })).toBeNull();
  await user.click(screen.getByRole('button', { name: 'Triggers' }));
  const sheet = screen.getByRole('dialog', { name: 'Triggers on orders' });
  expect(sheet).toHaveTextContent(
    'BEFORE UPDATE · FOR EACH ROW · public.touch() · enabled',
  );
  expect(
    within(sheet).getByLabelText('Definition of orders_touch'),
  ).toHaveTextContent('CREATE TRIGGER');
});

it('stages new and dropped indexes, never constraint indexes', async () => {
  const user = await structure();
  await user.click(screen.getByRole('radio', { name: 'Index' }));
  const grid = screen.getByRole('grid', { name: 'Indexes' });
  fireEvent.contextMenu(within(grid).getByText('orders_pkey'));
  expect(screen.getByRole('menuitem', { name: 'Delete index' })).toBeDisabled();
  await user.keyboard('{Escape}');
  fireEvent.contextMenu(within(grid).getByText('orders_name_idx'));
  await user.click(screen.getByRole('menuitem', { name: 'Delete index' }));
  await user.click(screen.getByRole('button', { name: 'Add index' }));
  const sheet = screen.getByRole('dialog', { name: 'Add index' });
  await user.click(within(sheet).getByRole('checkbox', { name: 'name' }));
  await user.click(within(sheet).getByRole('button', { name: 'Stage index' }));
  expect(grid).toHaveTextContent('(automatic name)');
  expect(screen.getByText('2 staged structure changes')).toBeVisible();
});

it('stages the table comment from the inspector and renames from the Name field', async () => {
  const user = await structure();
  const comment = screen.getByRole('textbox', { name: 'comment' });
  await user.clear(comment);
  await user.type(comment, 'All orders');
  await user.tab();
  const name = screen.getByRole('textbox', { name: 'Table name' });
  await user.clear(name);
  await user.type(name, 'purchases{Enter}');
  expect(screen.getByText('2 staged structure changes')).toBeVisible();
  await user.click(screen.getByRole('button', { name: 'Review & commit' }));
  expect(vi.mocked(tableAlter).mock.lastCall?.[0].changes).toEqual([
    { kind: 'tableComment', comment: 'All orders' },
    { kind: 'renameTable', name: 'purchases' },
  ]);
});

it('lists the partitions of a partitioned table', async () => {
  vi.mocked(tableDetail).mockResolvedValue({
    ...sampleDetail,
    partitions: [
      {
        schema: 'public',
        name: 'orders_2024',
        bound: "FOR VALUES FROM ('2024-01-01') TO ('2025-01-01')",
        partitioned: false,
        rowEstimate: 1200,
      },
    ],
  });
  await structure();
  const partitions = await screen.findByRole('table', { name: 'Partitions' });
  expect(partitions).toHaveTextContent('orders_2024');
  expect(partitions).toHaveTextContent("FOR VALUES FROM ('2024-01-01')");
  expect(partitions).toHaveTextContent('1,200');
  vi.mocked(tableDetail).mockResolvedValue(sampleDetail);
});

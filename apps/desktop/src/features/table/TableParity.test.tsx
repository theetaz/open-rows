// SPDX-License-Identifier: Apache-2.0
import { invalidateLinks } from './links/linkMetadata';
import { useState } from 'react';
import { act, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import type { CellRange } from '@openrows/grid';
import { schemaDiagram, tableDetail, tablePage, tableSql } from '@openrows/ipc';
import type { PageRequest, QueryEventDto } from '@openrows/ipc';
import type { Connections } from '../connections';
import { sampleDetail } from '../../test/structureStub';
import { profile } from '../../test/ipcStub';
import { QueryContent, useQueryTabs } from '../query';
import { openTableView } from './open';
import { RowDetails } from './RowDetails';
import type { SelectedRow } from './row';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);
/** Column 0 holds the row number (1-based), column 1 the same text in every row. */
vi.mock('../../results/displayWindow', () => ({
  displayWindow: vi.fn(
    (request: {
      rowStart: number;
      rowCount: number;
      colStart: number;
      colCount: number;
    }) =>
      Promise.resolve({
        cells: Array.from(
          { length: request.rowCount * request.colCount },
          (_, index) => {
            const row = request.rowStart + Math.floor(index / request.colCount);
            const column = request.colStart + (index % request.colCount);
            return {
              text: column === 0 ? String(row + 1) : 'same',
              state: 'normal',
            };
          },
        ),
        columns: request.colCount,
        convertMs: 0,
      }),
  ),
}));
vi.mock('./TableGrid', () => ({
  TableGrid: (props: {
    result: { id: string };
    onRow: (row: number) => void;
    onLink?: (cell: { row: number; column: number }) => void;
    links?: ReadonlySet<string>;
    onSelection?: (
      cell: { row: number; column: number },
      ranges: CellRange[],
    ) => void;
  }) => (
    <div role="grid" aria-label={`Grid ${props.result.id}`}>
      <span>Linked: {[...(props.links ?? [])].join(',')}</span>
      <button
        type="button"
        onClick={() => {
          props.onLink?.({ row: 6, column: 0 });
        }}
      >
        Follow key
      </button>
      <button
        type="button"
        onClick={() => {
          props.onSelection?.({ row: 0, column: 0 }, [
            { r0: 0, r1: 1, c0: 0, c1: 1 },
          ]);
        }}
      >
        Select two rows
      </button>
      <button
        type="button"
        onClick={() => {
          props.onSelection?.({ row: 0, column: 0 }, [
            { r0: 0, r1: 1, c0: 0, c1: 0 },
          ]);
        }}
      >
        Select ids
      </button>
    </div>
  ),
}));

const column = (name: string, primaryKey = false) => ({
  name,
  dataType: 'integer',
  nullable: !primaryKey,
  primaryKey,
});
vi.mocked(schemaDiagram).mockResolvedValue({
  tables: [
    { name: 'users', columns: [column('id', true), column('name')] },
    { name: 'orgs', columns: [column('id', true)] },
    { name: 'notes', columns: [column('id', true), column('user_id')] },
  ],
  foreignKeys: [
    {
      name: 'users_org',
      table: 'users',
      columns: ['id'],
      targetTable: 'orgs',
      targetColumns: ['id'],
    },
    {
      name: 'notes_user',
      table: 'notes',
      columns: ['user_id'],
      targetTable: 'users',
      targetColumns: ['id'],
    },
  ],
  truncated: false,
  limit: 400,
});

vi.mocked(tableDetail).mockResolvedValue({
  ...sampleDetail,
  constraints: [
    {
      name: 'users_org',
      kind: 'foreign',
      definition: '',
      columns: ['id'],
      references: { schema: 'public', table: 'orgs', columns: ['id'] },
    },
  ],
});

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

let emit: (event: QueryEventDto) => void = () => undefined;
let pages = 0;

beforeEach(() => {
  pages = 0;
  vi.mocked(tablePage).mockReset();
  vi.mocked(tablePage).mockImplementation((_request, onEvent) => {
    emit = onEvent;
    pages += 1;
    return Promise.resolve({ queryId: `q${String(pages)}`, sql: 'SELECT' });
  });
});

/** Stream a finished page of `rows` rows with `id` and `name`. */
function page(rows: number): void {
  const resultId = `r${String(pages)}`;
  act(() => {
    for (const event of [
      { type: 'statementStarted', statement: 0 },
      {
        type: 'resultBegin',
        statement: 0,
        resultId,
        columns: [
          { name: 'id', sourceType: 'int4' },
          { name: 'name', sourceType: 'text' },
        ],
      },
      { type: 'resultEnd', statement: 0, resultId, rows, truncated: false },
      { type: 'finished', outcome: 'completed', elapsedMs: 3 },
    ] satisfies QueryEventDto[])
      emit(event);
  });
}

function lastRequest(): PageRequest {
  const call = vi.mocked(tablePage).mock.lastCall;
  if (!call) throw new Error('no page requested');
  return call[0];
}

function Harness(): React.JSX.Element {
  const tabs = useQueryTabs();
  const [row, setRow] = useState<SelectedRow | null>(null);
  const scope = { connectionId: connection.id, database: 'orders' };
  const open = (name: string, schema = 'public'): void => {
    openTableView(tabs, scope, {
      database: 'orders',
      schema,
      name,
      kind: 'table',
    });
  };
  return (
    <>
      <button
        type="button"
        onClick={() => {
          open('users');
        }}
      >
        Open users
      </button>
      <QueryContent
        connections={connections}
        tabs={tabs}
        database="orders"
        onCell={vi.fn()}
        onRow={setRow}
        onOpenTable={(table) => {
          open(table.name, table.schema);
        }}
      />
      {row && <RowDetails row={row} />}
    </>
  );
}

async function mount(): Promise<ReturnType<typeof userEvent.setup>> {
  const user = userEvent.setup();
  render(
    <ThemeProvider>
      <Harness />
    </ThemeProvider>,
  );
  await user.click(screen.getByRole('button', { name: 'Open users' }));
  await screen.findByRole('region', { name: 'users table' });
  await waitFor(() => {
    expect(tablePage).toHaveBeenCalledTimes(1);
  });
  return user;
}

it('builds a raw SQL filter with a live WHERE check, saves it and restores it next session', async () => {
  const user = await mount();
  page(300);
  await user.click(screen.getByRole('button', { name: 'Filters' }));
  const bar = within(await screen.findByRole('search', { name: 'Filters' }));
  await user.selectOptions(bar.getByLabelText('Column of filter 1'), '');
  expect(bar.queryByLabelText('Operator of filter 1')).toBeNull();
  vi.mocked(tableSql).mockResolvedValue({
    sql: 'SELECT * FROM "public"."users" WHERE (id > 5) LIMIT 1 OFFSET 0',
    params: [],
  });
  await user.type(bar.getByLabelText('SQL of filter 1'), 'id > 5');
  const where = await bar.findByLabelText('Generated WHERE');
  await waitFor(() => {
    expect(where).toHaveTextContent('WHERE (id > 5)');
  });
  vi.mocked(tableSql).mockRejectedValue(
    new Error('the raw SQL filter cannot contain comments'),
  );
  await user.type(bar.getByLabelText('SQL of filter 1'), ' -- x');
  expect(await bar.findByRole('alert')).toHaveTextContent(
    'cannot contain comments',
  );
  await user.clear(bar.getByLabelText('SQL of filter 1'));
  await user.type(bar.getByLabelText('SQL of filter 1'), 'id > 5{Enter}');
  expect(lastRequest().query.filters).toEqual([
    { column: '', operator: 'raw', value: 'id > 5', second: '' },
  ]);
  await user.click(bar.getByRole('button', { name: 'Save…' }));
  await user.type(bar.getByLabelText('Saved filter name'), 'Big ids{Enter}');
  expect(bar.getByLabelText('Saved filters')).toHaveValue('Big ids');
  page(5);
  document.body.innerHTML = '';
  vi.mocked(tablePage).mockClear();
  await mount();
  expect(lastRequest().query.groups?.[0]?.filters).toEqual([
    { column: '', operator: 'raw', value: 'id > 5', second: '' },
  ]);
  const restored = within(screen.getByRole('search', { name: 'Filters' }));
  expect(restored.getByLabelText('SQL of filter 1')).toHaveValue('id > 5');
  expect(
    within(restored.getByLabelText('Saved filters')).getByRole('option', {
      name: 'Big ids',
    }),
  ).toBeInTheDocument();
});

it('follows a foreign key arrow to the referenced row in a filtered tab', async () => {
  const user = await mount();
  page(300);
  expect(await screen.findByText('Linked: id')).toBeVisible();
  await user.click(screen.getByRole('button', { name: 'Follow key' }));
  await screen.findByRole('region', { name: 'orgs table' });
  await waitFor(() => {
    expect(lastRequest().query).toMatchObject({
      target: { table: 'orgs' },
      filters: [{ column: 'id', operator: 'equal', value: '7', second: '' }],
      combinator: 'and',
    });
  });
});

it('sums a numeric selection in the footer and shows several rows in the inspector', async () => {
  const user = await mount();
  page(300);
  await user.click(screen.getByRole('button', { name: 'Select ids' }));
  expect(
    await screen.findByRole('status', { name: 'Selection statistics' }),
  ).toHaveTextContent('Count: 2 · Sum: 3 · Avg: 1.5 · Min: 1 · Max: 2');
  await user.click(screen.getByRole('button', { name: 'Select two rows' }));
  const fields = await screen.findByRole('list', {
    name: 'Fields of the selected rows',
  });
  expect(screen.getByText(/2 rows selected/)).toBeVisible();
  expect(within(fields).getByLabelText('id')).toHaveAttribute(
    'placeholder',
    'Multiple values',
  );
  expect(within(fields).getByLabelText('name')).toHaveValue('same');
});

it('shows the focused row as a form with previous and next', async () => {
  const user = await mount();
  page(3);
  await user.click(screen.getByRole('button', { name: 'Row form' }));
  const form = within(await screen.findByRole('region', { name: 'Row form' }));
  expect(form.getByRole('status', { name: 'Current row' })).toHaveTextContent(
    'Row 1',
  );
  expect(await form.findByLabelText('Value of id')).toHaveValue('1');
  expect(form.getByRole('button', { name: 'Previous row' })).toBeDisabled();
  await user.click(form.getByRole('button', { name: 'Next row' }));
  await waitFor(() => {
    expect(form.getByLabelText('Value of id')).toHaveValue('2');
  });
  await user.click(form.getByRole('button', { name: 'Show grid' }));
  expect(await screen.findByRole('grid')).toBeVisible();
});

it('shows and follows a cross-schema foreign key with its qualified row filters', async () => {
  invalidateLinks(connection.id);
  vi.mocked(tableDetail).mockResolvedValueOnce({
    ...sampleDetail,
    constraints: [
      {
        name: 'users_customers',
        kind: 'foreign',
        definition: '',
        columns: ['id'],
        references: { schema: 'b', table: 'customers', columns: ['id'] },
      },
    ],
  });
  const user = await mount();
  page(300);
  expect(await screen.findByText('Linked: id')).toBeVisible();
  await user.click(screen.getByRole('button', { name: 'Follow key' }));
  await screen.findByRole('region', { name: 'customers table' });
  await waitFor(() => {
    expect(lastRequest().query).toMatchObject({
      target: { schema: 'b', table: 'customers' },
      filters: [{ column: 'id', operator: 'equal', value: '7', second: '' }],
    });
  });
});

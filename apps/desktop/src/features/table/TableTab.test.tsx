// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import {
  tableCount,
  tableEditInfo,
  tableEditRow,
  tablePage,
  tableSql,
} from '@openrows/ipc';
import type { PageRequest, QueryEventDto } from '@openrows/ipc';
import type { Connections } from '../connections';
import { profile } from '../../test/ipcStub';
import { QueryContent, useQueryTabs } from '../query';
import { openTableView } from './open';
import { RowDetails } from './RowDetails';
import type { SelectedRow } from './row';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);
vi.mock('../../results/displayWindow', () => ({
  displayWindow: vi.fn(() =>
    Promise.resolve({
      cells: [
        { text: '7', state: 'normal' },
        { text: 'EMPTY', state: 'empty' },
      ],
      columns: 2,
      convertMs: 0,
    }),
  ),
}));
vi.mock('./TableGrid', () => ({
  TableGrid: (props: {
    result: { id: string; rows: number; columns: { name: string }[] };
    onHeader: (column: number) => void;
    onRow: (row: number) => void;
  }) => (
    <div role="grid" aria-label={`Grid ${props.result.id}`}>
      {props.result.columns.map((column, index) => (
        <button
          key={column.name}
          type="button"
          onClick={() => {
            props.onHeader(index);
          }}
        >
          Sort {column.name}
        </button>
      ))}
      <button
        type="button"
        onClick={() => {
          props.onRow(6);
        }}
      >
        Select row 7
      </button>
    </div>
  ),
}));

const connection = profile('orders-local');
const connections = {
  active: connection,
  statusOf: () => ({
    state: 'connected',
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

/** Stream a finished page of `rows` rows. */
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
  return (
    <>
      <button
        type="button"
        onClick={() => {
          openTableView(tabs, scope, {
            database: 'orders',
            schema: 'public',
            name: 'users',
            kind: 'table',
          });
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

const shown = (): HTMLElement =>
  screen.getByRole('status', { name: 'Rows shown' });

it('opens a table tab and pages through rows with buttons and ⌘[ ⌘]', async () => {
  const user = await mount();
  expect(screen.getByRole('tab', { name: /users/ })).toBeVisible();
  expect(lastRequest()).toMatchObject({
    sessionId: 'session-1',
    offset: 0,
    limit: 300,
  });
  page(300);
  expect(shown()).toHaveTextContent('1–300 of ~1,811 rows');
  await user.click(screen.getByRole('button', { name: 'Next page (⌘])' }));
  expect(lastRequest().offset).toBe(300);
  page(11);
  expect(shown()).toHaveTextContent('301–311 of ~1,811 rows');
  expect(screen.getByRole('button', { name: 'Next page (⌘])' })).toBeDisabled();
  fireEvent.keyDown(window, { key: '[', metaKey: true });
  await waitFor(() => {
    expect(lastRequest().offset).toBe(0);
  });
  page(300);
  fireEvent.keyDown(window, { key: ']', metaKey: true });
  await waitFor(() => {
    expect(lastRequest().offset).toBe(300);
  });
  const insert = screen.getByRole('button', { name: 'Add row' });
  expect(insert).toBeDisabled();
  expect(insert).toHaveAttribute(
    'title',
    'Read-only: no supported non-null unique key',
  );
});

it('sorts server-side from header clicks: ascending, descending, none', async () => {
  const user = await mount();
  page(300);
  await user.click(screen.getByRole('button', { name: 'Sort name' }));
  expect(lastRequest().query.sort).toEqual({
    column: 'name',
    descending: false,
  });
  page(300);
  await user.click(screen.getByRole('button', { name: 'Sort name' }));
  expect(lastRequest().query.sort).toEqual({
    column: 'name',
    descending: true,
  });
  page(300);
  await user.click(screen.getByRole('button', { name: 'Sort name' }));
  expect(lastRequest().query.sort).toBeNull();
});

it('applies, previews and clears filters with values as data', async () => {
  const user = await mount();
  page(300);
  fireEvent.keyDown(window, { key: 'f', metaKey: true });
  const bar = await screen.findByRole('search', { name: 'Filters' });
  const filters = within(bar);
  await waitFor(() => {
    expect(filters.getByLabelText('Value of filter 1')).toHaveFocus();
  });
  await user.selectOptions(
    filters.getByLabelText('Column of filter 1'),
    'name',
  );
  await user.selectOptions(
    filters.getByLabelText('Operator of filter 1'),
    'contains',
  );
  const value = filters.getByLabelText('Value of filter 1');
  await user.type(value, "'; DROP TABLE users; --{Enter}");
  expect(lastRequest()).toMatchObject({
    offset: 0,
    query: {
      filters: [
        {
          column: 'name',
          operator: 'contains',
          value: "'; DROP TABLE users; --",
          second: '',
        },
      ],
    },
  });
  page(1);
  expect(shown()).toHaveTextContent('1–1 rows');
  await user.click(filters.getByRole('button', { name: 'SQL' }));
  expect(tableSql).toHaveBeenCalled();
  expect(await screen.findByLabelText('Generated SQL')).toHaveTextContent(
    'SELECT 1',
  );
  await user.click(filters.getByRole('button', { name: 'Clear' }));
  expect(lastRequest().query.filters).toEqual([]);
});

it('shows the selected row in the details panel with field search', async () => {
  const user = await mount();
  page(300);
  await user.click(screen.getByRole('button', { name: 'Select row 7' }));
  const list = await screen.findByLabelText('Row fields');
  expect(within(list).getByLabelText('Value of id')).toHaveValue('7');
  expect(list).toHaveTextContent('name');
  expect(screen.getByText('users · row 7')).toBeVisible();
  await user.type(screen.getByLabelText('Search for field'), 'nam');
  expect(list).not.toHaveTextContent('id');
  const empty = within(list).getByLabelText('Value of name');
  expect(empty).toHaveValue('');
  expect(empty).toHaveAttribute('placeholder', 'EMPTY');
  expect(empty).toHaveAttribute('readonly');
});

it('edits inspector fields in place: staged values show and the row stays selected', async () => {
  const facts = { comment: null, primaryKey: false, enumLabels: null };
  vi.mocked(tableEditInfo).mockResolvedValueOnce({
    key: ['id'],
    protected: [],
    readOnly: false,
    columns: [
      {
        ...facts,
        name: 'id',
        dataType: 'integer',
        nullable: false,
        default: null,
      },
      {
        ...facts,
        name: 'name',
        dataType: 'text',
        nullable: true,
        default: "'anon'::text",
      },
    ],
  });
  const user = await mount();
  page(300);
  await user.click(screen.getByRole('button', { name: 'Select row 7' }));
  const name = await screen.findByLabelText('Value of name');
  await waitFor(() => {
    expect(
      screen.getByRole('button', { name: 'Value options for name' }),
    ).toBeVisible();
  });
  expect(name).toHaveAttribute('readonly');
  await user.click(name);
  await waitFor(() => {
    expect(name).not.toHaveAttribute('readonly');
  });
  expect(tableEditRow).toHaveBeenCalledWith(expect.anything(), { id: '7' });
  await user.type(name, 'Ada{Enter}');
  await waitFor(() => {
    expect(name.closest('[data-state]')).toHaveAttribute(
      'data-state',
      'changed',
    );
  });
  expect(name).toHaveValue('Ada');
  expect(screen.getByText('users · row 7')).toBeVisible();
  await user.click(
    screen.getByRole('button', { name: 'Value options for name' }),
  );
  await user.click(screen.getByRole('menuitem', { name: 'DEFAULT' }));
  await waitFor(() => {
    expect(screen.getByLabelText('Value of name')).toHaveAttribute(
      'placeholder',
      'DEFAULT',
    );
  });
  expect(screen.getByLabelText('Value of name')).toHaveValue('');
});

it('counts exactly on demand and shows structure and indexes', async () => {
  vi.mocked(tableCount).mockResolvedValue({ kind: 'counted', rows: 1811 });
  const user = await mount();
  page(300);
  await user.click(screen.getByRole('button', { name: 'Count' }));
  expect(tableCount).toHaveBeenCalledWith(
    expect.objectContaining({ sessionId: 'session-1' }),
  );
  await waitFor(() => {
    expect(shown()).toHaveTextContent('1–300 of 1,811 rows');
  });
  await user.click(screen.getByRole('radio', { name: 'Structure' }));
  expect(
    await screen.findByRole('grid', { name: 'Columns' }),
  ).toHaveTextContent('Display name');
  await user.click(screen.getByRole('radio', { name: 'Index' }));
  expect(screen.getByRole('grid', { name: 'Indexes' })).toHaveTextContent(
    'orders_pkey',
  );
  await user.click(screen.getByRole('radio', { name: 'Data' }));
  await user.click(screen.getByRole('button', { name: 'Columns' }));
  await user.click(screen.getByRole('checkbox', { name: 'name' }));
  expect(lastRequest().query.columns).toEqual(['id']);
});

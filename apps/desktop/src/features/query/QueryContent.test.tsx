// SPDX-License-Identifier: Apache-2.0
// Load the lazy tab before interactions so transform time is not UI latency.
import './QueryTab';
import '../export/ExportSheet';
import { act, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import {
  cancelQuery,
  executeQuery,
  releaseResults,
  startExport,
} from '@openrows/ipc';
import type { ExecuteReply, QueryEventDto, StatementDto } from '@openrows/ipc';
import type { Connections } from '../connections';
import { profile } from '../../test/ipcStub';
import { runTabCommand } from '../commands';
import { QueryContent } from './QueryContent';
import { useQueryTabs } from './useQueryTabs';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);
vi.mock('./QueryEditor', async () => ({
  QueryEditor: (await import('./testEditor')).TextareaEditor,
}));
vi.mock('./ResultGrid', () => ({
  ResultGrid: (props: {
    result: { id: string; rows: number };
    onExport?: () => void;
  }) => (
    <div role="grid" aria-label={`Grid ${props.result.id}`}>
      {props.result.rows} rows
      {props.onExport && <button onClick={props.onExport}>Export…</button>}
    </div>
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

let emit: (event: QueryEventDto) => void = () => undefined;
function reply(next: ExecuteReply): void {
  vi.mocked(executeQuery).mockImplementation((_request, onEvent) => {
    emit = onEvent;
    return Promise.resolve(next);
  });
}
function statement(
  index: number,
  sql: string,
  overrides: Partial<StatementDto> = {},
): StatementDto {
  return {
    index,
    sql,
    kind: 'read',
    dangerous: false,
    verdict: 'allow',
    ...overrides,
  };
}
async function send(...events: QueryEventDto[]): Promise<void> {
  await act(async () => {
    for (const event of events) emit(event);
    await vi.dynamicImportSettled();
  });
}
const finished = (
  outcome: 'completed' | 'failed' | 'cancelled',
): QueryEventDto => ({
  type: 'finished',
  outcome,
  elapsedMs: 12,
});
const result = (
  statement: number,
  id: string,
  rows: number,
): QueryEventDto[] => [
  { type: 'statementStarted', statement },
  {
    type: 'resultBegin',
    statement,
    resultId: id,
    columns: [{ name: 'n', sourceType: 'int4' }],
  },
  { type: 'resultEnd', statement, resultId: id, rows, truncated: false },
  { type: 'statementDone', statement, elapsedMs: 3 },
];

function Harness(): React.JSX.Element {
  const tabs = useQueryTabs();
  return (
    <QueryContent
      connections={connections}
      tabs={tabs}
      database="orders"
      onCell={vi.fn()}
      onRow={vi.fn()}
    />
  );
}

async function openTab(
  sql: string,
): Promise<ReturnType<typeof userEvent.setup>> {
  const user = userEvent.setup();
  render(
    <ThemeProvider>
      <Harness />
    </ThemeProvider>,
  );
  await user.click(screen.getByRole('button', { name: 'New query tab (⌘T)' }));
  await act(async () => {
    await vi.dynamicImportSettled();
  });
  const editor = await screen.findByRole('textbox', { name: 'SQL editor' });
  await user.type(editor, sql);
  return user;
}

/** Run All from the Run button's dropdown. */
async function runAll(user: ReturnType<typeof userEvent.setup>): Promise<void> {
  await user.click(screen.getByRole('button', { name: 'Run options' }));
  await user.click(
    within(screen.getByRole('menu', { name: 'Run options' })).getByRole(
      'menuitem',
      { name: /Run All/ },
    ),
  );
}

const status = (): HTMLElement =>
  screen.getByRole('status', { name: 'Query status' });

beforeEach(() => {
  reply({
    kind: 'started',
    queryId: 'q1',
    statements: [statement(0, 'SELECT 1')],
  });
});

it('runs the statement at the cursor and shows its rows and status', async () => {
  const user = await openTab('SELECT 1;\nSELECT 2;');
  const editor = screen.getByRole<HTMLTextAreaElement>('textbox', {
    name: 'SQL editor',
  });
  editor.setSelectionRange(13, 13);
  await user.click(
    screen.getByRole('button', { name: 'Run current statement' }),
  );
  expect(executeQuery).toHaveBeenCalledWith(
    {
      sessionId: 'session-1',
      sql: 'SELECT 1;\nSELECT 2;',
      mode: 'current',
      cursor: 13,
      confirmToken: null,
      maxRows: null,
      returnOnError: true,
      params: null,
      autoLimit: null,
      confirmMultiple: false,
    },
    expect.any(Function),
  );
  await send(...result(0, 'r1', 1), finished('completed'));
  await act(async () => {
    await vi.dynamicImportSettled();
  });
  expect(
    await screen.findByRole('grid', { name: 'Grid r1' }),
  ).toHaveTextContent('1 rows');
  expect(status()).toHaveTextContent('Done · 1 row · 12 ms');
  expect(screen.getByRole('tab', { name: /SQL Query 1/ })).toBeVisible();
});

it('runs all statements with ⇧⌘↵ and shows one result tab per result', async () => {
  reply({
    kind: 'started',
    queryId: 'q1',
    statements: [
      statement(0, 'SELECT 1'),
      statement(1, 'UPDATE t SET a = 1 WHERE b', { kind: 'write' }),
      statement(2, 'SELECT 2'),
    ],
  });
  const user = await openTab('SELECT 1; UPDATE t SET a = 1 WHERE b; SELECT 2');
  await user.keyboard('{Meta>}{Shift>}{Enter}{/Shift}{/Meta}');
  expect(vi.mocked(executeQuery).mock.calls[0]?.[0]).toMatchObject({
    mode: 'all',
    cursor: null,
  });
  await send(
    ...result(0, 'r1', 5),
    { type: 'statementStarted', statement: 1 },
    { type: 'rowsAffected', statement: 1, count: 3 },
    { type: 'notice', statement: 1, level: 'warning', message: 'careful' },
    { type: 'statementDone', statement: 1, elapsedMs: 1 },
    ...result(2, 'r2', 2),
    finished('completed'),
  );
  const tabs = screen.getByRole('tablist', { name: 'Result sets' });
  expect(within(tabs).getAllByRole('tab')).toHaveLength(2);
  expect(await screen.findByRole('grid', { name: 'Grid r2' })).toBeVisible();
  await user.click(within(tabs).getByRole('tab', { name: /Result 1/ }));
  expect(await screen.findByRole('grid', { name: 'Grid r1' })).toBeVisible();
  expect(status()).toHaveTextContent(
    '3 statements · 7 rows · 3 rows affected · 1 notice · 12 ms',
  );
  expect(screen.getByRole('list', { name: 'Messages' })).toHaveTextContent(
    'WARNING: careful',
  );
});

it('runs only the selected text when text is selected', async () => {
  const user = await openTab('SELECT 1; SELECT 2;');
  const editor = screen.getByRole<HTMLTextAreaElement>('textbox', {
    name: 'SQL editor',
  });
  editor.setSelectionRange(10, 18);
  await user.click(
    screen.getByRole('button', { name: 'Run current statement' }),
  );
  expect(vi.mocked(executeQuery).mock.calls[0]?.[0]).toMatchObject({
    sql: 'SELECT 2',
    mode: 'selection',
  });
});

it('turns Run into Cancel while a query runs', async () => {
  const user = await openTab('SELECT pg_sleep(30)');
  expect(screen.queryByRole('button', { name: 'Cancel query' })).toBeNull();
  await user.click(
    screen.getByRole('button', { name: 'Run current statement' }),
  );
  await send({ type: 'statementStarted', statement: 0 });
  expect(
    screen.queryByRole('button', { name: 'Run current statement' }),
  ).toBeNull();
  await user.click(screen.getByRole('button', { name: 'Run options' }));
  const options = screen.getByRole('menu', { name: 'Run options' });
  for (const name of [/Run All/, /Run Current/])
    expect(within(options).getByRole('menuitem', { name })).toBeDisabled();
  await user.keyboard('{Escape}');
  await user.click(screen.getByRole('button', { name: 'Cancel query' }));
  expect(cancelQuery).toHaveBeenCalledWith('q1');
  await send({ type: 'cancelled', statement: 0 }, finished('cancelled'));
  expect(status()).toHaveTextContent('Cancelled · 12 ms');
  expect(screen.queryByRole('button', { name: 'Cancel query' })).toBeNull();
  expect(
    screen.getByRole('button', { name: 'Run current statement' }),
  ).toBeEnabled();
});

it('shows server errors with their SQLSTATE', async () => {
  const user = await openTab('SELECT * FROM nope');
  await user.click(
    screen.getByRole('button', { name: 'Run current statement' }),
  );
  await send(
    { type: 'statementStarted', statement: 0 },
    {
      type: 'failed',
      statement: 0,
      error: {
        kind: 'server',
        message: 'relation "nope" does not exist',
        code: '42P01',
      },
    },
    finished('failed'),
  );
  expect(screen.getByRole('list', { name: 'Messages' })).toHaveTextContent(
    'ERROR 42P01: relation "nope" does not exist',
  );
  expect(status()).toHaveTextContent(
    'Failed: relation "nope" does not exist (SQLSTATE 42P01)',
  );
});

it('asks for confirmation and resends with the token only after consent', async () => {
  reply({
    kind: 'confirmationRequired',
    token: 't-1',
    statements: [
      statement(0, 'SELECT 1'),
      statement(1, 'DROP TABLE orders', {
        kind: 'ddl',
        dangerous: true,
        verdict: 'confirm',
      }),
    ],
  });
  const user = await openTab('SELECT 1; DROP TABLE orders;');
  await runAll(user);
  const dialog = await screen.findByRole('dialog', {
    name: 'Run statements that change data?',
  });
  const list = within(dialog).getByRole('list', {
    name: 'Statements to confirm',
  });
  expect(within(list).getAllByRole('listitem')).toHaveLength(1);
  expect(list).toHaveTextContent('Dangerous');
  expect(list).toHaveTextContent('DROP TABLE orders');
  expect(dialog).toHaveTextContent('1 other statement also run');
  await user.click(within(dialog).getByRole('button', { name: 'Cancel' }));
  expect(screen.queryByRole('dialog')).toBeNull();
  expect(executeQuery).toHaveBeenCalledOnce();

  await runAll(user);
  reply({ kind: 'started', queryId: 'q2', statements: [] });
  const again = await screen.findByRole('dialog', {
    name: 'Run statements that change data?',
  });
  await user.click(
    within(again).getByRole('button', { name: 'Run statements' }),
  );
  expect(executeQuery).toHaveBeenCalledTimes(3);
  expect(vi.mocked(executeQuery).mock.calls[2]?.[0]).toMatchObject({
    mode: 'all',
    confirmToken: 't-1',
  });
});

it('reports read-only blocks without running anything', async () => {
  reply({
    kind: 'blocked',
    message: 'This connection is read-only: nothing was run.',
    statements: [
      statement(0, 'DELETE FROM t', {
        kind: 'write',
        dangerous: true,
        verdict: 'block',
      }),
    ],
  });
  const user = await openTab('DELETE FROM t');
  await user.click(
    screen.getByRole('button', { name: 'Run current statement' }),
  );
  expect(
    await screen.findByRole('list', { name: 'Messages' }),
  ).toHaveTextContent('read-only');
  expect(status()).toHaveTextContent('Blocked: read-only connection');
  expect(screen.queryByRole('dialog')).toBeNull();
});

it('frees results when a tab closes or re-runs', async () => {
  const user = await openTab('SELECT 1');
  await user.click(
    screen.getByRole('button', { name: 'Run current statement' }),
  );
  await send(...result(0, 'r1', 1), finished('completed'));
  await act(async () => {
    await vi.dynamicImportSettled();
  });
  await user.click(
    screen.getByRole('button', { name: 'Run current statement' }),
  );
  expect(releaseResults).toHaveBeenLastCalledWith(['r1']);
  await send(...result(0, 'r2', 1), finished('completed'));
  await user.click(screen.getByRole('button', { name: 'Close SQL Query 1' }));
  expect(releaseResults).toHaveBeenLastCalledWith(['r2']);
  expect(
    screen.getByRole('button', { name: 'New query tab (⌘T)' }),
  ).toBeVisible();
});

it('exports the shown result from the result store', async () => {
  const user = await openTab('SELECT 1');
  await user.click(
    screen.getByRole('button', { name: 'Run current statement' }),
  );
  expect(screen.queryByRole('button', { name: 'Export…' })).toBeNull();
  await send(...result(0, 'r1', 3), finished('completed'));
  await user.click(screen.getByRole('button', { name: 'Export…' }));
  const sheet = await screen.findByRole('dialog', { name: 'Export Result' });
  await user.click(within(sheet).getByRole('button', { name: 'Export…' }));
  expect(startExport).toHaveBeenCalledWith(
    expect.objectContaining({
      source: { kind: 'result', sessionId: 'session-1', resultId: 'r1' },
      fileName: 'result',
    }),
    expect.any(Function),
  );
});

it('Export results opens the export sheet of the shown result', async () => {
  const user = await openTab('SELECT 1');
  expect(runTabCommand('export-results')).toBe(true);
  expect(screen.queryByRole('dialog', { name: 'Export Result' })).toBeNull();
  await user.click(
    screen.getByRole('button', { name: 'Run current statement' }),
  );
  await send(...result(0, 'r1', 3), finished('completed'));
  await screen.findByRole('button', { name: 'Export…' });
  act(() => {
    runTabCommand('export-results');
  });
  expect(
    await screen.findByRole('dialog', { name: 'Export Result' }),
  ).toBeVisible();
});

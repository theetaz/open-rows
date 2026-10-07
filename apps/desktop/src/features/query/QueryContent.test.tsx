// SPDX-License-Identifier: Apache-2.0
import { act, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import { cancelQuery, executeQuery, releaseResults } from '@openrows/ipc';
import type { ExecuteReply, QueryEventDto, StatementDto } from '@openrows/ipc';
import type { Connections } from '../connections';
import { profile } from '../../test/ipcStub';
import { QueryContent } from './QueryContent';
import { useQueryTabs } from './useQueryTabs';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);
vi.mock('./QueryEditor', async () => ({
  QueryEditor: (await import('./testEditor')).TextareaEditor,
}));
vi.mock('./ResultGrid', () => ({
  ResultGrid: (props: { result: { id: string; rows: number } }) => (
    <div role="grid" aria-label={`Grid ${props.result.id}`}>
      {props.result.rows} rows
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
function send(...events: QueryEventDto[]): void {
  act(() => {
    for (const event of events) emit(event);
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
  const editor = await screen.findByRole('textbox', { name: 'SQL editor' });
  await user.type(editor, sql);
  return user;
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
    },
    expect.any(Function),
  );
  send(...result(0, 'r1', 1), finished('completed'));
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
  send(
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

it('cancels a running query', async () => {
  const user = await openTab('SELECT pg_sleep(30)');
  const cancel = screen.getByRole('button', { name: 'Cancel query' });
  expect(cancel).toBeDisabled();
  await user.click(
    screen.getByRole('button', { name: 'Run current statement' }),
  );
  send({ type: 'statementStarted', statement: 0 });
  expect(
    screen.getByRole('button', { name: 'Run current statement' }),
  ).toBeDisabled();
  await user.click(cancel);
  expect(cancelQuery).toHaveBeenCalledWith('q1');
  send({ type: 'cancelled', statement: 0 }, finished('cancelled'));
  expect(status()).toHaveTextContent('Cancelled · 12 ms');
  expect(cancel).toBeDisabled();
});

it('shows server errors with their SQLSTATE', async () => {
  const user = await openTab('SELECT * FROM nope');
  await user.click(
    screen.getByRole('button', { name: 'Run current statement' }),
  );
  send(
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
  await user.click(screen.getByRole('button', { name: 'Run all statements' }));
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

  await user.click(screen.getByRole('button', { name: 'Run all statements' }));
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
  send(...result(0, 'r1', 1), finished('completed'));
  await user.click(
    screen.getByRole('button', { name: 'Run current statement' }),
  );
  expect(releaseResults).toHaveBeenLastCalledWith(['r1']);
  send(...result(0, 'r2', 1), finished('completed'));
  await user.click(screen.getByRole('button', { name: 'Close SQL Query 1' }));
  expect(releaseResults).toHaveBeenLastCalledWith(['r2']);
  expect(
    screen.getByRole('button', { name: 'New query tab (⌘T)' }),
  ).toBeVisible();
});

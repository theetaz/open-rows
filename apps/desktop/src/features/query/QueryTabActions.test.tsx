// SPDX-License-Identifier: Apache-2.0
import { screen, within } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import { createSavedQuery, saveSqlFile, saveSqlFileAs } from '@openrows/ipc';
import type { ExecuteRequest } from '@openrows/ipc';
import { choose, chooseIn, fakeRuns, renderTab } from './testQueryTab';
import { idleRun } from './run';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);
vi.mock('./QueryEditor', async () => ({
  QueryEditor: (await import('./testEditor')).TextareaEditor,
}));

const editorText = (): string =>
  screen.getByRole<HTMLTextAreaElement>('textbox', { name: 'SQL editor' })
    .value;

it('switches the default run and its shortcut labels', async () => {
  const runs = fakeRuns();
  const { user } = renderTab(runs);
  await chooseIn(user, 'Run options', 'Default', /Run All/);
  const button = screen.getByRole('button', { name: 'Run all statements' });
  expect(button).toHaveTextContent('Run All');
  await user.click(button);
  expect(runs.execute).toHaveBeenCalledWith(
    'query-1',
    expect.objectContaining({ mode: 'all', sql: 'select 1' }),
  );
  await user.click(screen.getByRole('button', { name: 'Run options' }));
  const menu = screen.getByRole('menu', { name: 'Run options' });
  expect(
    within(menu).getByRole('menuitem', { name: /Run All/ }),
  ).toHaveTextContent('⌘↵');
  expect(
    within(menu).getByRole('menuitem', { name: /Run Current/ }),
  ).toHaveTextContent('⇧⌘↵');
});

it('sends the error policy and the parameter style with a run', async () => {
  const runs = fakeRuns();
  const { user } = renderTab(runs);
  await choose(user, 'Editor settings', /Return on Error/);
  await chooseIn(user, 'Editor settings', 'Query Params', /:name/);
  await user.click(
    screen.getByRole('button', { name: 'Run current statement' }),
  );
  expect(runs.execute).toHaveBeenCalledWith(
    'query-1',
    expect.objectContaining({
      returnOnError: false,
      params: { style: 'named', values: null },
    }),
  );
});

it('asks for parameter values, prefilled with the last ones, and binds them', async () => {
  localStorage.setItem(
    'openrows.editor',
    JSON.stringify({ queryParams: 'named' }),
  );
  const request: ExecuteRequest = {
    sessionId: 'session-1',
    sql: 'select :id',
    mode: 'current',
    cursor: 0,
    confirmToken: null,
    params: { style: 'named', values: null },
  };
  const runs = fakeRuns();
  runs.runOf = () => ({
    ...idleRun,
    status: 'parameters',
    parameters: { request, names: [':id'] },
  });
  const { user, tabs } = renderTab(runs, { tab: { params: { ':id': '7' } } });
  const field = screen.getByRole('textbox', { name: ':id' });
  expect(field).toHaveValue('7');
  await user.clear(field);
  await user.type(field, '8');
  await user.click(screen.getByRole('button', { name: 'Run' }));
  expect(tabs.setParams).toHaveBeenCalledWith('query-1', { ':id': '8' });
  expect(runs.execute).toHaveBeenCalledWith('query-1', {
    ...request,
    params: { style: 'named', values: [{ name: ':id', value: '8' }] },
  });
});

it('binds the tab to the file chosen in Save SQL As, then ⌘S saves to it', async () => {
  vi.mocked(saveSqlFileAs).mockResolvedValue({
    path: '/work/report.sql',
    name: 'report.sql',
  });
  const first = renderTab();
  await choose(first.user, 'Run options', /Save SQL As/);
  expect(saveSqlFileAs).toHaveBeenCalledWith('SQL Query 1', 'select 1');
  expect(first.tabs.bindFile).toHaveBeenCalledWith('query-1', {
    path: '/work/report.sql',
    name: 'report.sql',
    saved: 'select 1',
  });
  first.unmount();
  const file = { path: '/work/report.sql', name: 'report.sql', saved: '' };
  const { user, tabs } = renderTab(fakeRuns(), { tab: { file } });
  await user.click(screen.getByRole('textbox', { name: 'SQL editor' }));
  await user.keyboard('{Meta>}s{/Meta}');
  expect(saveSqlFile).toHaveBeenCalledWith('/work/report.sql', editorText());
  expect(tabs.bindFile).toHaveBeenCalledWith('query-1', {
    path: '/work/report.sql',
    name: 'report.sql',
    saved: 'select 1',
  });
});

it('adds the tab to Queries under the entered name', async () => {
  const { user } = renderTab();
  await choose(user, 'Run options', /Add to Queries/);
  const name = screen.getByRole('textbox', { name: 'Query name' });
  expect(name).toHaveValue('SQL Query 1');
  await user.clear(name);
  await user.type(name, 'Weekly{Enter}');
  expect(createSavedQuery).toHaveBeenCalledWith(
    'id-local',
    'Weekly',
    'select 1',
  );
  expect(screen.queryByRole('dialog', { name: 'Add to Queries' })).toBeNull();
});

it('beautifies with the chosen indent type', async () => {
  const { user } = renderTab();
  await chooseIn(user, 'Format options', 'Indent Type', /Tabular Right/);
  await user.click(screen.getByRole('button', { name: 'Beautify' }));
  expect(editorText()).toMatch(
    /^-- beautify postgres 2 spaces upper tabularRight\n/,
  );
});

it('shows one tab per result or only the last result', async () => {
  const runs = fakeRuns();
  const result = (id: string, statement: number, rows: number) => ({
    id,
    statement,
    columns: [],
    rows,
    done: false,
    truncated: false,
  });
  runs.runOf = () => ({
    ...idleRun,
    status: 'running',
    results: [result('r1', 0, 3), result('r2', 1, 0)],
  });
  const { user } = renderTab(runs);
  const sets = screen.getByRole('tablist', { name: 'Result sets' });
  expect(within(sets).getAllByRole('tab')).toHaveLength(2);
  await chooseIn(user, 'Editor settings', 'Result View Mode', /Last Result/);
  expect(screen.queryByRole('tablist', { name: 'Result sets' })).toBeNull();
  // The last result has no rows yet; one with rows would show its grid.
  expect(screen.getByText(/Receiving rows… 0 so far/)).toBeVisible();
});

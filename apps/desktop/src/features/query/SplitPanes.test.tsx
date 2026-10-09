// SPDX-License-Identifier: Apache-2.0
import { act, fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import { executeQuery } from '@openrows/ipc';
import type { Connections } from '../connections';
import { profile } from '../../test/ipcStub';
import { runPaneCommand, runTabCommand } from '../commands';
import { updateExecution } from './executionSettings';
import { QueryContent } from './QueryContent';
import { useQueryTabs } from './useQueryTabs';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);
vi.mock('./QueryEditor', async () => ({
  QueryEditor: (await import('./testEditor')).TextareaEditor,
}));

const connections = {
  active: profile('orders-local'),
  statusOf: () => ({
    state: 'connected',
    insecure: [],
    databases: [{ name: 'orders', state: 'open', sessionId: 'session-1' }],
  }),
  connect: vi.fn(),
} as unknown as Connections;

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

async function firstTab(): Promise<ReturnType<typeof userEvent.setup>> {
  const user = userEvent.setup();
  render(
    <ThemeProvider>
      <Harness />
    </ThemeProvider>,
  );
  await user.click(screen.getByRole('button', { name: 'New query tab (⌘T)' }));
  await user.type(
    await screen.findByRole('textbox', { name: 'SQL editor' }),
    'SELECT 1',
  );
  return user;
}

const pane = (title: RegExp): HTMLElement =>
  screen.getByRole('region', { name: title });

it('splits right and down, keeps the first tab, resizes and closes', async () => {
  const user = await firstTab();
  act(() => {
    runPaneCommand('split-right');
  });
  const first = pane(/Split pane: SQL Query 1/);
  const second = pane(/Split pane: SQL Query 2/);
  expect(
    within(first).getByRole('textbox', { name: 'SQL editor' }),
  ).toHaveValue('SELECT 1');
  expect(
    within(second).getByRole('textbox', { name: 'SQL editor' }),
  ).toHaveValue('');
  expect(
    within(first).getByText('orders-local · orders · SQL Query 1'),
  ).toBeVisible();
  expect(
    within(second).getByText('orders-local · orders · SQL Query 2'),
  ).toBeVisible();
  const divider = screen.getByRole('separator', { name: 'Resize split panes' });
  expect(divider).toHaveAttribute('aria-orientation', 'vertical');
  fireEvent.keyDown(divider, { key: 'ArrowRight' });
  expect(divider).toHaveAttribute('aria-valuenow', '55');
  fireEvent.keyDown(divider, { key: 'End' });
  expect(divider).toHaveAttribute('aria-valuenow', '80');
  act(() => {
    runPaneCommand('split-down');
  });
  expect(divider).toHaveAttribute('aria-orientation', 'horizontal');
  await user.click(screen.getByRole('button', { name: 'Close split pane' }));
  expect(screen.queryByRole('separator')).toBeNull();
  expect(screen.getAllByRole('textbox', { name: 'SQL editor' })).toHaveLength(
    1,
  );
});

it('focuses the other pane by keyboard and runs in the focused pane', async () => {
  vi.mocked(executeQuery).mockResolvedValue({
    kind: 'started',
    queryId: 'q1',
    statements: [],
  });
  updateExecution({ autoLimit: 100 });
  const user = await firstTab();
  act(() => {
    runPaneCommand('split-right');
  });
  const [left, right] = screen.getAllByRole('textbox', { name: 'SQL editor' });
  await user.type(right ?? document.body, 'SELECT 2');
  act(() => {
    runPaneCommand('focus-pane');
  });
  expect(left).toHaveFocus();
  act(() => {
    runTabCommand('run-no-limit');
  });
  expect(executeQuery).toHaveBeenLastCalledWith(
    expect.objectContaining({ sql: 'SELECT 1', autoLimit: null }),
    expect.any(Function),
  );
  act(() => {
    right?.focus();
    runTabCommand('run-current');
  });
  expect(executeQuery).toHaveBeenLastCalledWith(
    expect.objectContaining({ sql: 'SELECT 2', autoLimit: 100 }),
    expect.any(Function),
  );
});

it('fills the focused pane with the tab chosen in the tab strip', async () => {
  const user = await firstTab();
  act(() => {
    runPaneCommand('split-right');
  });
  act(() => {
    pane(/Split pane: SQL Query 1/).focus();
  });
  await user.click(screen.getByRole('tab', { name: /SQL Query 2/ }));
  expect(pane(/Split pane: SQL Query 2/)).toHaveAttribute(
    'data-pane',
    'pinned',
  );
  expect(pane(/Split pane: SQL Query 1/)).toHaveAttribute(
    'data-pane',
    'active',
  );
});

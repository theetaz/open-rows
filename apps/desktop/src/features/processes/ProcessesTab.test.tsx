// SPDX-License-Identifier: Apache-2.0
import { act, fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import {
  confirmDestructive,
  listProcesses,
  signalProcess,
} from '@openrows/ipc';
import type { ProfileDto } from '@openrows/ipc';
import { profile } from '../../test/ipcStub';
import { sampleProcess } from '../../test/processStub';
import { ProcessesTab } from './ProcessesTab';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);

const sleeper = sampleProcess(42, {
  query: 'SELECT pg_sleep(60)\n  -- nightly report',
  durationMs: 30_000,
  user: 'reporting',
});
const own = sampleProcess(7, {
  application: 'OpenRows metadata',
  current: true,
  app: true,
  durationMs: 0,
  query: 'SELECT pid FROM pg_stat_activity',
});
const idle = sampleProcess(99, { state: 'idle', durationMs: 5000 });

beforeEach(() => {
  vi.mocked(listProcesses).mockReset();
  vi.mocked(listProcesses).mockResolvedValue([own, idle, sleeper]);
  vi.mocked(signalProcess).mockClear();
  vi.mocked(confirmDestructive).mockReset();
  vi.mocked(confirmDestructive).mockResolvedValue(true);
});

afterEach(() => {
  vi.useRealTimers();
});

function renderTab(owner: ProfileDto = profile('orders-local')) {
  const onRow = vi.fn();
  render(
    <ProcessesTab profile={owner} database="orders" connected onRow={onRow} />,
  );
  return onRow;
}

/** The PIDs in display order. */
function pids(): string[] {
  const grid = screen.getByRole('grid', { name: 'Processes' });
  return within(grid)
    .getAllByRole('row')
    .slice(1)
    .map((row) => within(row).getAllByRole('gridcell')[0]?.textContent ?? '');
}

async function openMenu(pid: string): Promise<HTMLElement> {
  const cell = await screen.findByRole('gridcell', { name: pid });
  const row = cell.closest('tr');
  if (!row) throw new Error('no row');
  fireEvent.contextMenu(row, { clientX: 10, clientY: 10 });
  return screen.findByRole('menu');
}

it('lists processes longest first, filters them and sorts by a header', async () => {
  renderTab();
  expect(await screen.findByText('3 of 3 processes')).toBeInTheDocument();
  expect(pids()).toEqual(['42', '99', '7']);
  expect(screen.getByText('This list')).toBeInTheDocument();
  await userEvent.click(screen.getByRole('button', { name: 'PID' }));
  expect(pids()).toEqual(['7', '42', '99']);
  expect(screen.getByRole('columnheader', { name: /PID/ })).toHaveAttribute(
    'aria-sort',
    'ascending',
  );
  await userEvent.type(
    screen.getByRole('searchbox', { name: 'Filter processes' }),
    'pg_sleep',
  );
  expect(pids()).toEqual(['42']);
  expect(screen.getByText('1 of 3 processes')).toBeInTheDocument();
});

it('shows the selected process with its full query in the inspector', async () => {
  const onRow = renderTab();
  await userEvent.click(await screen.findByRole('gridcell', { name: '99' }));
  await userEvent.click(screen.getByRole('gridcell', { name: '42' }));
  const shown = onRow.mock.lastCall?.[0] as {
    table: string;
    fields: { name: string; text: string }[];
  };
  expect(shown.table).toBe('process 42 (reporting, psql)');
  expect(shown.fields.find((field) => field.name === 'Query')?.text).toBe(
    sleeper.query,
  );
});

it('cancels a query after the native confirmation, then reloads', async () => {
  renderTab();
  const menu = await openMenu('42');
  await userEvent.click(
    within(menu).getByRole('menuitem', { name: 'Cancel Query' }),
  );
  expect(confirmDestructive).toHaveBeenCalledWith(
    expect.objectContaining({ action: 'Cancel Query' }),
  );
  expect(signalProcess).toHaveBeenCalledWith({
    profileId: 'id-orders-local',
    database: 'orders',
    processId: 42,
    signal: 'cancelQuery',
    allowOwn: false,
  });
  expect(listProcesses).toHaveBeenCalledTimes(2);
});

it('sends nothing when the confirmation is declined', async () => {
  vi.mocked(confirmDestructive).mockResolvedValue(false);
  renderTab();
  const menu = await openMenu('42');
  await userEvent.click(
    within(menu).getByRole('menuitem', { name: 'Kill Process' }),
  );
  expect(confirmDestructive).toHaveBeenCalled();
  expect(signalProcess).not.toHaveBeenCalled();
});

it('kills the list’s own connection only after the sheet names it', async () => {
  vi.mocked(confirmDestructive).mockResolvedValue(null);
  renderTab();
  const menu = await openMenu('7');
  await userEvent.click(
    within(menu).getByRole('menuitem', { name: 'Kill Process' }),
  );
  const sheet = await screen.findByRole('dialog');
  expect(sheet).toHaveTextContent('this process list runs on');
  expect(signalProcess).not.toHaveBeenCalled();
  await userEvent.click(
    within(sheet).getByRole('button', { name: 'Kill Process' }),
  );
  expect(signalProcess).toHaveBeenCalledWith(
    expect.objectContaining({
      processId: 7,
      signal: 'terminate',
      allowOwn: true,
    }),
  );
});

it('copies the query and disables signals on read-only connections', async () => {
  const writeText = vi.fn(() => Promise.resolve());
  Object.assign(navigator, { clipboard: { writeText } });
  renderTab(
    profile('orders-local', {
      safety: 'readOnly',
    }),
  );
  const menu = await openMenu('42');
  expect(
    within(menu).getByRole('menuitem', { name: 'Cancel Query' }),
  ).toBeDisabled();
  expect(
    within(menu).getByRole('menuitem', { name: 'Kill Process' }),
  ).toBeDisabled();
  await userEvent.click(
    within(menu).getByRole('menuitem', { name: 'Copy Query' }),
  );
  expect(writeText).toHaveBeenCalledWith(sleeper.query);
  expect(signalProcess).not.toHaveBeenCalled();
});

it('refreshes on the chosen interval and stops when turned off', async () => {
  vi.useFakeTimers({ shouldAdvanceTime: true });
  renderTab();
  await screen.findByText('3 of 3 processes');
  const radio = (name: string): HTMLElement =>
    screen.getByRole('radio', { name });
  fireEvent.click(radio('2 s'));
  await act(() => vi.advanceTimersByTimeAsync(4100));
  expect(listProcesses).toHaveBeenCalledTimes(3);
  fireEvent.click(radio('Off'));
  await act(() => vi.advanceTimersByTimeAsync(10_000));
  expect(listProcesses).toHaveBeenCalledTimes(3);
});

it('shows why the list failed', async () => {
  vi.mocked(listProcesses).mockRejectedValue(new Error('permission denied'));
  renderTab();
  expect(await screen.findByRole('alert')).toHaveTextContent(
    'permission denied',
  );
});

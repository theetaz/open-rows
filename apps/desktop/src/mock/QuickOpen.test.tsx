// SPDX-License-Identifier: Apache-2.0
import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import axe from 'axe-core';
import { ThemeProvider } from '@openrows/ui';
import { searchCatalog } from '@openrows/ipc';
import { connectTo } from '../test/connect';
import { Workspace } from './Workspace';
import { COMMANDS } from '../features/commands';

vi.mock('@openrows/ipc', async () =>
  (await import('../test/ipcStub')).ipcStub(),
);
function mount(enabled = true): void {
  vi.stubGlobal('innerWidth', 1280);
  render(
    <ThemeProvider>
      <Workspace onGallery={vi.fn()} enabled={enabled} />
    </ThemeProvider>,
  );
}
async function key(key: string, shiftKey = false): Promise<void> {
  await act(async () => {
    fireEvent.keyDown(window, { key, metaKey: true, shiftKey });
    await import('./WorkspacePalettes');
  });
}
function choose(query: string): void {
  const input = within(screen.getByRole('dialog')).getByRole('combobox');
  fireEvent.change(input, { target: { value: query } });
  fireEvent.keyDown(input, { key: 'Enter' });
  act(() => {
    vi.advanceTimersByTime(160);
  });
}
/** Make orders-local the active connection once profiles have loaded. */
async function connect(): Promise<void> {
  await act(async () => {
    await Promise.resolve();
  });
  fireEvent.click(
    screen.getByRole('button', { name: 'Switch to orders-local' }),
  );
}
afterEach(() => {
  vi.useRealTimers();
});
it('⌘P searches the real name index, opens a query tab per object and refocuses an open one', async () => {
  mount();
  await connectTo();
  vi.useFakeTimers();
  await key('p');
  const dialog = within(screen.getByRole('dialog', { name: 'Quick open' }));
  expect(
    dialog.getByRole('option', {
      name: /customers.*Table · public · ~412K rows \(estimate\)/,
    }),
  ).toBeVisible();
  choose('cstm');
  expect(screen.getByRole('tab', { name: /customers/ })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  expect(screen.getByRole('tab', { name: /customers/ })).toHaveAttribute(
    'title',
    'orders-local · customers',
  );
  await key('p');
  choose('order_summary');
  expect(screen.getByRole('tab', { name: /order_summary/ })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  await key('p');
  choose('session_notes');
  await key('p');
  choose('cstm');
  expect(screen.getAllByRole('tab', { name: /customers/ })).toHaveLength(1);
  expect(screen.getByRole('tab', { name: /customers/ })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  await key('p');
  choose('calc_tax');
  expect(screen.getByRole('tab', { name: /calc_tax/ })).toHaveAttribute(
    'title',
    'orders-local · calc_tax(total numeric)',
  );
  expect(screen.getByRole('combobox', { name: 'Schema' })).toHaveValue(
    'public',
  );
  await key('p');
  choose('session_notes');
  expect(screen.getAllByRole('tab', { name: /session_notes/ })).toHaveLength(1);
  expect(screen.getByRole('tab', { name: /session_notes/ })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  expect(screen.getByRole('combobox', { name: 'Schema' })).toHaveValue(
    'archive',
  );
  expect(searchCatalog).toHaveBeenCalledOnce();
  expect(screen.getAllByRole('tab')).toHaveLength(4);
  fireEvent.click(screen.getByRole('button', { name: 'Close session_notes' }));
  expect(screen.queryByRole('tab', { name: /session_notes/ })).toBeNull();
});
it('⌘P without a connection explains why nothing is listed', async () => {
  mount();
  await key('p');
  expect(
    within(screen.getByRole('dialog', { name: 'Quick open' })).getByText(
      'Choose a connection to browse its objects.',
    ),
  ).toBeVisible();
});
it('⌘⇧P lists every command and executes sidebar, inspector, query, refresh and safe mode actions', async () => {
  vi.useFakeTimers();
  mount();
  await connect();
  await key('p', true);
  const palette = within(
    screen.getByRole('dialog', { name: 'Command palette' }),
  );
  expect(palette.getAllByRole('option')).toHaveLength(COMMANDS.length);
  expect(
    palette.getByRole('option', { name: /Toggle sidebar.*⌃⌘S/ }),
  ).toBeVisible();
  choose('Toggle sidebar');
  expect(
    screen.getByRole('button', { name: 'Toggle sidebar (⌃⌘S)' }),
  ).toHaveAttribute('aria-pressed', 'false');
  for (const command of [
    'Toggle inspector',
    'New query tab',
    'Refresh',
    'Toggle safe mode',
  ]) {
    await key('p', true);
    choose(command);
  }
  expect(
    screen.getByRole('button', { name: 'Toggle inspector (⌥⌘I)' }),
  ).toHaveAttribute('aria-pressed', 'false');
  expect(screen.getByRole('tab', { name: /SQL Query 1/ })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  expect(screen.getByRole('button', { name: 'Safe mode' })).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  expect(screen.getByText('Catalog refreshed (1)')).toBeInTheDocument();
  await key('p', true);
  choose('Open database');
  expect(screen.getByRole('dialog', { name: 'Open database' })).toBeVisible();
});
it('toolbar palette returns focus on Escape and exact shortcuts are suspended while modal or disabled', async () => {
  vi.useFakeTimers();
  mount();
  const opener = screen.getByRole('button', { name: 'Command palette' });
  opener.focus();
  await act(async () => {
    fireEvent.click(opener);
    await import('./WorkspacePalettes');
  });
  await key('t');
  expect(screen.queryByRole('tab', { name: /SQL Query 1/ })).toBeNull();
  fireEvent.keyDown(within(screen.getByRole('dialog')).getByRole('combobox'), {
    key: 'Escape',
  });
  act(() => {
    vi.advanceTimersByTime(160);
  });
  expect(opener).toHaveFocus();
  fireEvent.keyDown(window, { key: 'p', metaKey: true, altKey: true });
  expect(screen.queryByRole('dialog')).toBeNull();
});
it('disabled workspace ignores palette shortcuts', async () => {
  mount(false);
  await key('k');
  await key('p');
  await key('p', true);
  expect(screen.queryByRole('dialog')).toBeNull();
});
it.each(['database', 'quick', 'commands'])(
  'axe: %s palette has clean semantics',
  async (mode) => {
    mount();
    await key(mode === 'database' ? 'k' : 'p', mode === 'commands');
    const result = await axe.run(screen.getByRole('dialog'), {
      rules: { 'color-contrast': { enabled: false } },
    });
    expect(result.violations).toEqual([]);
  },
);

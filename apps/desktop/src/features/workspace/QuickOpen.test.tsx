// SPDX-License-Identifier: Apache-2.0
import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import axe from 'axe-core';
import { ThemeProvider } from '@openrows/ui';
import { searchCatalog } from '@openrows/ipc';
import { connectTo } from '../../test/connect';
import { Workspace } from './Workspace';
import { COMMANDS } from '../commands';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
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
/** Type into ⌘P, wait for the index search to answer, then press Return. */
async function chooseObject(query: string): Promise<void> {
  const input = within(screen.getByRole('dialog')).getByRole('combobox');
  fireEvent.change(input, { target: { value: query } });
  await act(async () => {
    for (let tick = 0; tick < 5; tick++) await Promise.resolve();
  });
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
it('⌘P searches the Rust name index per keystroke, opens a tab per relation, refocuses an open one and edits routines', async () => {
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
  await chooseObject('cstm');
  expect(screen.getByRole('tab', { name: /customers/ })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  expect(screen.getByRole('tab', { name: /customers/ })).toHaveAttribute(
    'title',
    'orders-local · customers',
  );
  await key('p');
  await chooseObject('order_summary');
  expect(screen.getByRole('tab', { name: /order_summary/ })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  await key('p');
  await chooseObject('session_notes');
  await key('p');
  await chooseObject('cstm');
  expect(screen.getAllByRole('tab', { name: /customers/ })).toHaveLength(1);
  expect(screen.getByRole('tab', { name: /customers/ })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  await key('p');
  await chooseObject('calc_tax');
  const editor = screen.getByRole('dialog', { name: 'Edit function calc_tax' });
  expect(screen.queryByRole('tab', { name: /calc_tax/ })).toBeNull();
  fireEvent.keyDown(editor, { key: 'Escape' });
  expect(screen.queryByRole('dialog', { name: /calc_tax/ })).toBeNull();
  expect(screen.getByRole('combobox', { name: 'Schema' })).toHaveValue(
    'public',
  );
  await key('p');
  await chooseObject('session_notes');
  expect(screen.getAllByRole('tab', { name: /session_notes/ })).toHaveLength(1);
  expect(screen.getByRole('tab', { name: /session_notes/ })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  expect(screen.getByRole('combobox', { name: 'Schema' })).toHaveValue(
    'archive',
  );
  expect(searchCatalog).toHaveBeenCalledWith('id-orders-local', 'orders', {
    query: 'cstm',
    schema: null,
    limit: 100,
    fresh: false,
  });
  // customers, order_summary and session_notes; the routine opened its editor.
  expect(screen.getAllByRole('tab')).toHaveLength(3);
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

// SPDX-License-Identifier: Apache-2.0
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import { Workspace } from './Workspace';
import { DataGrid } from './DataGrid';
import type { OrderRow } from './data/rows';

async function mount(): Promise<ReturnType<typeof userEvent.setup>> {
  vi.stubGlobal('innerWidth', 1280);
  await act(async () => {
    await Promise.resolve();
    render(
      <ThemeProvider>
        <Workspace onGallery={vi.fn()} />
      </ThemeProvider>,
    );
  });
  return userEvent.setup();
}
it('closed inspector, review, palettes, menus and tooltips leave no content DOM', async () => {
  const user = await mount();
  expect(document.querySelector('dialog')).toBeNull();
  expect(document.querySelector('[role="menu"]')).toBeNull();
  expect(document.querySelector('[role="tooltip"]')).toBeNull();
  await screen.findByRole('textbox', { name: 'Ask the assistant' });
  await user.click(
    screen.getByRole('button', { name: 'Toggle inspector (⌥⌘I)' }),
  );
  await waitFor(() => {
    expect(
      document.querySelector('[aria-label="Ask the assistant"]'),
    ).toBeNull();
  });
  await user.click(screen.getByRole('button', { name: 'Review & Commit' }));
  await screen.findByRole('dialog', { name: 'Review & Commit' });
  await user.click(screen.getByRole('button', { name: 'Cancel' }));
  expect(document.querySelector('dialog')).toBeNull();
  await user.click(screen.getByRole('button', { name: 'Workspace menu' }));
  await user.keyboard('{Escape}');
  expect(document.querySelector('[role="menu"]')).toBeNull();
  const tile = screen.getByRole('button', { name: 'Switch to events' });
  await user.hover(tile);
  expect(screen.getByRole('tooltip')).toBeVisible();
  await user.unhover(tile);
  expect(document.querySelector('[role="tooltip"]')).toBeNull();
});
it.each(['k', 'p', 'P'])(
  'palette %s unmounts at the exit duration',
  async (key) => {
    await mount();
    vi.useFakeTimers();
    try {
      await act(async () => {
        fireEvent.keyDown(window, {
          key,
          metaKey: true,
          shiftKey: key === 'P',
        });
        await import('./WorkspacePalettes');
      });
      fireEvent.keyDown(
        screen.getByRole('combobox', {
          name: /Switch database|Quick open|Command palette/,
        }),
        { key: 'Escape' },
      );
      act(() => {
        vi.advanceTimersByTime(159);
      });
      expect(document.querySelector('dialog')).not.toBeNull();
      act(() => {
        vi.advanceTimersByTime(1);
      });
      expect(document.querySelector('dialog')).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  },
);
it('inactive tabs and databases retain SQL drafts and table state without content DOM', async () => {
  const user = await mount();
  await user.click(screen.getByRole('button', { name: 'Remove filter' }));
  await user.click(screen.getByRole('button', { name: 'New tab' }));
  const editor = await screen.findByRole('textbox', { name: 'SQL editor' });
  await user.clear(editor);
  await user.type(editor, 'SELECT 42;');
  expect(document.querySelector('[role="grid"]')).toBeNull();
  await user.click(screen.getByRole('tab', { name: 'orders' }));
  expect(editor.isConnected).toBe(false);
  expect(screen.queryByRole('button', { name: 'Remove filter' })).toBeNull();
  await user.click(screen.getByRole('tab', { name: 'SQL Query 1' }));
  expect(
    await screen.findByRole('textbox', { name: 'SQL editor' }),
  ).toHaveValue('SELECT 42;');
  await user.keyboard('{Meta>}k{/Meta}');
  const input = await screen.findByRole('combobox', {
    name: 'Switch database',
  });
  await user.type(input, 'orders_archive');
  await user.keyboard('{Enter}');
  await screen.findByRole('button', {
    name: 'Opened databases (orders_archive)',
  });
  expect(document.querySelector('[aria-label="SQL editor"]')).toBeNull();
  await user.click(
    screen.getByRole('button', { name: 'Opened databases (orders_archive)' }),
  );
  await user.click(
    screen.getByRole('menuitemradio', { name: 'Activate database orders' }),
  );
  expect(
    await screen.findByRole('textbox', { name: 'SQL editor' }),
  ).toHaveValue('SELECT 42;');
});
it('PERF: 10000 rows mount only the viewport plus overscan; scroll and keyboard reach the end', () => {
  const data = Array.from({ length: 10000 }, (_, index): OrderRow => [
    index,
    5512,
    'pending',
    '24.50',
    'USD',
    'Sep 30, 12:10',
    null,
  ]);
  render(
    <DataGrid rows={data} selected={0} pending={false} onSelect={vi.fn()} />,
  );
  const grid = screen.getByRole('grid');
  Object.defineProperty(grid, 'clientHeight', { value: 260 });
  fireEvent.scroll(grid, { target: { scrollTop: 26000 } });
  expect(grid.querySelectorAll('[data-row]')).toHaveLength(16);
  expect(grid.querySelector('[data-row="997"]')).not.toBeNull();
  expect(grid.querySelector('[data-row="0"]')).toBeNull();
  expect(grid).not.toHaveAttribute('aria-activedescendant');
  fireEvent.keyDown(grid, { key: 'End' });
  expect(grid.querySelector('[data-row="9999"]')).not.toBeNull();
  expect(grid.querySelectorAll('[data-row]').length).toBeLessThanOrEqual(16);
  fireEvent.keyDown(grid, { key: 'Home' });
  expect(grid.querySelector('[data-row="0"]')).not.toBeNull();
});

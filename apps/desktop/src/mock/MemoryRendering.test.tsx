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
import { connectTo } from '../test/connect';
import { Workspace } from './Workspace';

vi.mock('@openrows/ipc', async () =>
  (await import('../test/ipcStub')).ipcStub(),
);

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
it('closed inspector, palettes, menus and tooltips leave no content DOM', async () => {
  const user = await mount();
  expect(document.querySelector('dialog')).toBeNull();
  expect(document.querySelector('[role="menu"]')).toBeNull();
  expect(document.querySelector('[role="tooltip"]')).toBeNull();
  await screen.findByText('Select a cell in a result to see its value.');
  await user.click(
    screen.getByRole('button', { name: 'Toggle inspector (⌥⌘I)' }),
  );
  await waitFor(() => {
    expect(
      screen.queryByText('Select a cell in a result to see its value.'),
    ).toBeNull();
  });
  await connectTo();
  await user.click(
    screen.getByRole('button', { name: 'Opened databases (orders)' }),
  );
  await user.keyboard('{Escape}');
  expect(document.querySelector('[role="menu"]')).toBeNull();
  const tile = await screen.findByRole('button', {
    name: 'Switch to events-dev',
  });
  await user.hover(tile);
  expect(screen.getByRole('tooltip')).toBeVisible();
  await user.unhover(tile);
  expect(document.querySelector('[role="tooltip"]')).toBeNull();
});
it('the ⌘K open-database sheet unmounts on Escape', async () => {
  await mount();
  await act(async () => {
    fireEvent.keyDown(window, { key: 'k', metaKey: true });
    await import('./WorkspacePalettes');
  });
  fireEvent.keyDown(
    screen.getByRole('combobox', { name: 'Search for database' }),
    { key: 'Escape' },
  );
  expect(document.querySelector('dialog')).toBeNull();
});
it.each(['p', 'P'])('palette %s unmounts at the exit duration', async (key) => {
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
        name: /Quick open|Command palette/,
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
});
it('inactive query tabs keep their SQL without editor DOM', async () => {
  const user = await mount();
  await connectTo();
  await user.click(screen.getByRole('radio', { name: 'Assistant' }));
  await user.click(await screen.findByRole('button', { name: 'Open in tab' }));
  const editor = await screen.findByRole('textbox', { name: 'SQL editor' });
  expect(editor).toHaveTextContent('SELECT currency');
  await user.click(screen.getByRole('button', { name: 'New tab' }));
  await waitFor(() => {
    expect(editor.isConnected).toBe(false);
  });
  expect(document.querySelectorAll('.cm-editor')).toHaveLength(1);
  await user.click(screen.getByRole('tab', { name: /SQL Query 1/ }));
  await waitFor(() => {
    expect(
      screen.getByRole('textbox', { name: 'SQL editor' }),
    ).toHaveTextContent('SELECT currency');
  });
  expect(document.querySelectorAll('.cm-editor')).toHaveLength(1);
});

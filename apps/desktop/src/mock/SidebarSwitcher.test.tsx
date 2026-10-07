// SPDX-License-Identifier: Apache-2.0
import { act, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import { Workspace } from './Workspace';

vi.mock('@openrows/ipc', async () =>
  (await import('../test/ipcStub')).ipcStub(),
);
afterEach(() => {
  vi.useRealTimers();
});
it('sidebar menu opens the existing switcher and returns focus to its persistent selector', async () => {
  vi.useFakeTimers();
  render(
    <ThemeProvider>
      <Workspace onGallery={vi.fn()} />
    </ThemeProvider>,
  );
  const opener = screen.getByRole('button', {
    name: 'Opened databases (orders)',
  });
  opener.focus();
  fireEvent.click(opener);
  await act(async () => {
    fireEvent.click(screen.getByRole('menuitem', { name: /Open database/ }));
    await import('./WorkspacePalettes');
  });
  expect(screen.queryByRole('menu')).toBeNull();
  const input = screen.getByRole('combobox', {
    name: 'Switch database',
  });
  expect(input).toHaveFocus();
  fireEvent.change(input, { target: { value: 'orders_archive' } });
  fireEvent.keyDown(input, { key: 'Enter' });
  act(() => {
    vi.advanceTimersByTime(160);
  });
  expect(
    screen.getByRole('button', { name: 'Opened databases (orders_archive)' }),
  ).toHaveFocus();
  fireEvent.click(opener);
  expect(screen.getAllByRole('menuitemradio')).toHaveLength(2);
  await act(async () => {
    fireEvent.keyDown(document.activeElement ?? document.body, {
      key: 'k',
      metaKey: true,
    });
    await import('./WorkspacePalettes');
  });
  expect(screen.queryByRole('menu')).toBeNull();
  expect(
    screen.getByRole('combobox', { name: 'Switch database' }),
  ).toHaveFocus();
});

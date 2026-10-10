// SPDX-License-Identifier: Apache-2.0
import { act, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, expect, it, vi } from 'vitest';
import { emitTo } from '@tauri-apps/api/event';
import { App } from './App';

vi.mock('@openrows/ipc', async () =>
  (await import('../test/ipcStub')).ipcStub(),
);
vi.mock('@tauri-apps/api/event', () => ({ emitTo: vi.fn() }));

beforeEach(() => {
  vi.mocked(emitTo).mockReset().mockResolvedValue();
});

it('signals the main window once after the first commit', () => {
  render(<App />);
  expect(emitTo).toHaveBeenCalledExactlyOnceWith(
    'main',
    'openrows:first-paint-ready',
  );
});

it('reports a failed native reveal handshake', async () => {
  vi.mocked(emitTo).mockRejectedValue(new Error('Permission denied'));
  render(<App />);
  await act(async () => {
    await Promise.resolve();
  });
  expect(screen.getByRole('alert')).toHaveTextContent(
    'Unable to show the native window',
  );
});

async function openGallery(
  user: ReturnType<
    (typeof import('@testing-library/user-event'))['default']['setup']
  >,
): Promise<void> {
  await user.click(screen.getByRole('button', { name: 'Command palette' }));
  await user.click(
    await screen.findByRole('option', { name: /Open component gallery/ }),
  );
}
it('defaults to workspace, opens the gallery from the command palette and returns by Back/Escape', async () => {
  const { default: userEvent } = await import('@testing-library/user-event');
  const user = userEvent.setup();
  render(<App />);
  await waitFor(() => {
    expect(screen.getByText(/open a query tab/)).toBeVisible();
  });
  await openGallery(user);
  expect(
    await screen.findByRole('heading', { name: 'Made for the desktop.' }),
  ).toBeVisible();
  expect(screen.queryByText(/open a query tab/)).toBeNull();
  await user.keyboard('{Meta>}t{/Meta}');
  await user.click(screen.getByRole('button', { name: 'Back to workspace' }));
  expect(screen.queryByRole('tab', { name: /SQL Query 1/ })).toBeNull();
  expect(screen.getByText(/open a query tab/)).toBeVisible();
  await openGallery(user);
  // The gallery loads lazily; Escape before it mounts would be lost on slow runners.
  await screen.findByRole('heading', { name: 'Made for the desktop.' });
  await user.keyboard('{Escape}');
  expect(await screen.findByText(/open a query tab/)).toBeVisible();
});
it('opens settings by gear or shortcut, preserves the workspace and returns focus', async () => {
  const { default: userEvent } = await import('@testing-library/user-event');
  const user = userEvent.setup();
  render(<App />);
  const workspace = await screen.findByRole('navigation', {
    name: 'Workspaces',
  });
  const opener = screen.getByRole('button', { name: 'Settings' });
  await user.click(opener);
  expect(await screen.findByRole('dialog', { name: 'Settings' })).toBeVisible();
  expect(workspace.isConnected).toBe(true);
  await user.click(screen.getByRole('button', { name: 'Done' }));
  expect(workspace).toBeVisible();
  expect(opener).toHaveFocus();
  await user.keyboard('{Meta>},{/Meta}');
  expect(await screen.findByRole('dialog', { name: 'Settings' })).toBeVisible();
  await user.keyboard('{Escape}');
  expect(screen.queryByRole('dialog', { name: 'Settings' })).toBeNull();
});

// SPDX-License-Identifier: Apache-2.0
import { act, render, screen } from '@testing-library/react';
import { beforeEach, expect, it, vi } from 'vitest';
import { emitTo } from '@tauri-apps/api/event';
import { App } from './App';

vi.mock('@openrows/ipc', () => ({ getAppInfo: () => new Promise(() => {}) }));
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

it('defaults to workspace, opens the gallery from the rail and returns by Back/Escape', async () => {
  const { default: userEvent } = await import('@testing-library/user-event');
  const user = userEvent.setup();
  render(<App />);
  expect(screen.getByRole('grid', { name: 'Orders data' })).toBeVisible();
  await user.click(screen.getByRole('button', { name: 'Component gallery' }));
  expect(
    await screen.findByRole('heading', { name: 'Made for the desktop.' }),
  ).toBeVisible();
  expect(document.querySelector('[role="grid"]')).toBeNull();
  await user.keyboard('{Meta>}t{/Meta}');
  await user.click(screen.getByRole('button', { name: 'Back to workspace' }));
  expect(screen.queryByRole('tab', { name: /SQL Query 1/ })).toBeNull();
  expect(screen.getByRole('grid')).toBeVisible();
  await user.click(screen.getByRole('button', { name: 'Component gallery' }));
  await user.keyboard('{Escape}');
  expect(screen.getByRole('grid')).toBeVisible();
});

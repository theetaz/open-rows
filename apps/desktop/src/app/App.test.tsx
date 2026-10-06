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

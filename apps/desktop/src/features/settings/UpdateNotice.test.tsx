// SPDX-License-Identifier: Apache-2.0
import { beforeEach, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { UpdateNotice } from './UpdateNotice';
import { updateGeneral } from './general';
import { relaunch } from '@tauri-apps/plugin-process';

const update = vi.hoisted(() => ({
  version: '0.2.0',
  body: '<script>release notes</script>',
  download: vi.fn().mockResolvedValue(undefined),
  install: vi.fn().mockResolvedValue(undefined),
  close: vi.fn().mockResolvedValue(undefined),
}));
vi.mock('./updater', () => ({
  useUpdate: () => ({ update, message: 'Update available' }),
  checkUpdate: vi.fn(),
  dismissUpdate: vi.fn(),
}));
vi.mock('@tauri-apps/plugin-process', () => ({
  relaunch: vi.fn().mockResolvedValue(undefined),
}));

beforeEach(() => {
  vi.clearAllMocks();
  updateGeneral({ updateNotifications: true });
});

it('blocks installation while the workspace has tabs or connections', () => {
  render(<UpdateNotice canInstall={false} />);
  expect(
    screen.getByRole('button', { name: 'Install and restart' }),
  ).toBeDisabled();
  expect(
    screen.getByText(
      'Close all tabs and disconnect connections before installing.',
    ),
  ).toBeInTheDocument();
  expect(
    screen.getByText('<script>release notes</script>'),
  ).toBeInTheDocument();
  expect(document.querySelector('script')).toBeNull();
});

it('downloads, verifies through the plugin, installs, then restarts on approval', async () => {
  const showModal = vi.fn();
  HTMLDialogElement.prototype.showModal = showModal;
  render(<UpdateNotice canInstall />);
  fireEvent.click(screen.getByRole('button', { name: 'Install and restart' }));
  await waitFor(() => {
    expect(relaunch).toHaveBeenCalledOnce();
  });
  expect(update.download).toHaveBeenCalledOnce();
  expect(update.install).toHaveBeenCalledOnce();
  expect(showModal).toHaveBeenCalledOnce();
});

it('does not install or restart after a rejected download/signature', async () => {
  update.download.mockRejectedValueOnce(new Error('Invalid update signature'));
  render(<UpdateNotice canInstall />);
  fireEvent.click(screen.getByRole('button', { name: 'Install and restart' }));
  expect(await screen.findByRole('alert')).toHaveTextContent(
    'Invalid update signature',
  );
  expect(update.install).not.toHaveBeenCalled();
  expect(relaunch).not.toHaveBeenCalled();
});

it('rechecks workspace safety after download before replacing the app', async () => {
  let finish: (() => void) | undefined;
  update.download.mockImplementationOnce(
    () =>
      new Promise<void>((resolve) => {
        finish = resolve;
      }),
  );
  const view = render(<UpdateNotice canInstall />);
  fireEvent.click(screen.getByRole('button', { name: 'Install and restart' }));
  view.rerender(<UpdateNotice canInstall={false} />);
  finish?.();
  expect(await screen.findByRole('alert')).toHaveTextContent(
    'Workspace changed.',
  );
  expect(update.install).not.toHaveBeenCalled();
  expect(relaunch).not.toHaveBeenCalled();
  view.rerender(<UpdateNotice canInstall />);
  fireEvent.click(screen.getByRole('button', { name: 'Install and restart' }));
  await waitFor(() => {
    expect(relaunch).toHaveBeenCalledOnce();
  });
  expect(update.download).toHaveBeenCalledOnce();
});

it('stops and frees a late download when update consent is revoked', async () => {
  let finish: (() => void) | undefined;
  update.download.mockImplementationOnce(
    () =>
      new Promise<void>((resolve) => {
        finish = resolve;
      }),
  );
  render(<UpdateNotice canInstall />);
  fireEvent.click(screen.getByRole('button', { name: 'Install and restart' }));
  updateGeneral({ updateNotifications: false });
  finish?.();
  await waitFor(() => {
    expect(update.close).toHaveBeenCalledOnce();
  });
  expect(update.install).not.toHaveBeenCalled();
  expect(relaunch).not.toHaveBeenCalled();
});

it('frees late downloads and never installs after the notice is unmounted', async () => {
  let finish: (() => void) | undefined;
  update.download.mockImplementationOnce(
    () =>
      new Promise<void>((resolve) => {
        finish = resolve;
      }),
  );
  const view = render(<UpdateNotice canInstall />);
  fireEvent.click(screen.getByRole('button', { name: 'Install and restart' }));
  view.unmount();
  finish?.();
  await waitFor(() => {
    expect(update.close).toHaveBeenCalledOnce();
  });
  expect(update.install).not.toHaveBeenCalled();
  expect(relaunch).not.toHaveBeenCalled();
});

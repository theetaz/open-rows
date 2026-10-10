// SPDX-License-Identifier: Apache-2.0
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import { onSecurityChanged, securityStatus, unlockApp } from '@openrows/ipc';
import { ThemeProvider } from '@openrows/ui';
import { LockBoundary } from './LockBoundary';
import { SafetySelector } from './SafetySelector';
import { SecurityPane } from './SecurityPane';
import { parseSecurity } from './securitySettings';
import { ConfirmRun } from '../query/ConfirmRun';
import { COMMANDS } from '../commands/registry';
vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);

it('offers six policies and disables unavailable authentication with a visible fallback', async () => {
  vi.mocked(securityStatus).mockResolvedValue({
    locked: false,
    available: false,
    notice: 'OS authentication is unavailable.',
  });
  const changed = vi.fn();
  render(<SafetySelector value="confirmAll" onChange={changed} />);
  await screen.findByText(/OS authentication is unavailable/);
  expect(screen.getAllByRole('option')).toHaveLength(6);
  expect(
    screen.getByRole('option', { name: 'Authenticate all' }),
  ).toBeDisabled();
  expect(
    screen.getByRole('option', { name: 'Authenticate writes' }),
  ).toBeDisabled();
  fireEvent.change(screen.getByRole('combobox'), {
    target: { value: 'readOnly' },
  });
  expect(changed).toHaveBeenCalledWith('readOnly');
});

it('confirms reads with classification and connection/database target', () => {
  render(
    <ThemeProvider>
      <ConfirmRun
        connection="Test / orders"
        statements={[
          {
            index: 0,
            sql: 'SELECT 1',
            kind: 'read',
            dangerous: false,
            verdict: 'confirm',
          },
        ]}
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
      />
    </ThemeProvider>,
  );
  expect(screen.getByText('Test / orders')).toBeVisible();
  expect(screen.getByText('Read')).toBeVisible();
  expect(screen.getByText('SELECT 1')).toBeVisible();
});

it('hides mounted data while locked, blocks palette shortcuts and keeps failed unlock locked', async () => {
  vi.mocked(securityStatus).mockResolvedValue({
    locked: true,
    available: true,
    notice: null,
  });
  vi.mocked(unlockApp).mockRejectedValue(new Error('Authentication cancelled'));
  let refresh: (locked: boolean) => void = () => undefined;
  vi.mocked(onSecurityChanged).mockImplementation((listener) => {
    refresh = listener;
    return Promise.resolve(() => undefined);
  });
  const dialog = document.createElement('dialog');
  dialog.setAttribute('open', '');
  Object.defineProperty(dialog, 'close', {
    value: vi.fn(() => {
      dialog.removeAttribute('open');
    }),
  });
  Object.defineProperty(dialog, 'showModal', {
    value: vi.fn(() => {
      dialog.setAttribute('open', '');
    }),
  });
  document.body.append(dialog);
  render(
    <ThemeProvider>
      <LockBoundary>
        <p>Private rows</p>
      </LockBoundary>
    </ThemeProvider>,
  );
  await screen.findByRole('button', { name: 'Unlock OpenRows' });
  expect(screen.getByText('Private rows')).not.toBeVisible();
  expect(dialog).not.toHaveAttribute('open');
  const event = new KeyboardEvent('keydown', {
    key: 'P',
    metaKey: true,
    shiftKey: true,
    cancelable: true,
  });
  window.dispatchEvent(event);
  expect(event.defaultPrevented).toBe(true);
  fireEvent.click(screen.getByRole('button', { name: 'Unlock OpenRows' }));
  await screen.findByRole('alert');
  expect(screen.getByText('Private rows')).not.toBeVisible();
  vi.mocked(securityStatus).mockResolvedValue({
    locked: false,
    available: true,
    notice: null,
  });
  act(() => {
    refresh(false);
  });
  await waitFor(() => {
    expect(screen.getByText('Private rows')).toBeVisible();
  });
  expect(dialog).toHaveAttribute('open');
  dialog.remove();
});

it('disables lock settings without native authentication and validates defaults', async () => {
  vi.mocked(securityStatus).mockResolvedValue({
    locked: false,
    available: false,
    notice: 'OS authentication is unavailable.',
  });
  render(
    <ThemeProvider>
      <SecurityPane />
    </ThemeProvider>,
  );
  await screen.findByText('OS authentication is unavailable.');
  expect(
    screen.getByRole('switch', { name: 'Lock on startup' }),
  ).toBeDisabled();
  expect(
    screen.getByRole('combobox', { name: 'Lock after inactivity' }),
  ).toBeDisabled();
  expect(parseSecurity(null)).toEqual({
    lockOnStartup: false,
    lockAfterInactivityMinutes: null,
  });
  expect(parseSecurity({ lockAfterInactivityMinutes: 2 })).toEqual({
    lockOnStartup: false,
    lockAfterInactivityMinutes: null,
  });
  expect(COMMANDS.find(({ id }) => id === 'lock-app')?.shortcut).toBe(
    'Ctrl+Cmd+L',
  );
});

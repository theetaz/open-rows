// SPDX-License-Identifier: Apache-2.0
import { fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import {
  createProfile,
  parseConnectionUrl,
  testConnection,
  updateProfile,
} from '@openrows/ipc';
import { ThemeProvider } from '@openrows/ui';
import { profile, server } from '../../test/ipcStub';
import { ConnectionSheet } from './ConnectionSheet';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);

const URL_WITH_SECRET = 'postgres://app:pw-synthetic@db.test:6543/orders';

function mount(existing?: ReturnType<typeof profile>) {
  const handlers = {
    onClose: vi.fn(),
    onSaved: vi.fn(),
    onDelete: vi.fn(),
    onDuplicate: vi.fn(),
  };
  render(
    <ThemeProvider>
      <ConnectionSheet profile={existing} {...handlers} />
    </ThemeProvider>,
  );
  return { user: userEvent.setup(), handlers };
}

const PARSED = {
  host: 'db.test',
  port: 6543,
  database: 'orders',
  user: 'app',
  tlsMode: 'prefer' as const,
  passwordProvided: false,
  engine: 'postgres' as const,
  file: null,
};

it('fills the fields from a pasted URL at once, masks its password and saves it as typed', async () => {
  vi.mocked(parseConnectionUrl).mockResolvedValue(PARSED);
  const saved = profile('orders');
  vi.mocked(createProfile).mockResolvedValue(saved);
  const { user, handlers } = mount();
  const url = screen.getByRole('textbox', { name: 'URL' });
  expect(url).toHaveFocus();
  await user.clear(url);
  await user.paste(URL_WITH_SECRET);
  expect(parseConnectionUrl).toHaveBeenLastCalledWith(
    'postgres://app@db.test:6543/orders',
  );
  expect(url).toHaveValue('postgres://app:••••@db.test:6543/orders');
  expect(await screen.findByDisplayValue('db.test')).toBeVisible();
  expect(screen.getByRole('textbox', { name: 'Port' })).toHaveValue('6543');
  expect(screen.getByRole('textbox', { name: 'Name' })).toHaveValue('orders');
  const password = screen.getByLabelText('Password');
  expect(password).toHaveAttribute('type', 'password');
  expect(password).toHaveValue('pw-synthetic');
  await user.click(screen.getByRole('button', { name: 'Save' }));
  expect(createProfile).toHaveBeenCalledWith(
    expect.objectContaining({
      host: 'db.test',
      port: 6543,
      user: 'app',
      password: { kind: 'set', value: 'pw-synthetic' },
    }),
  );
  expect(handlers.onSaved).toHaveBeenCalledWith(saved);
});

it('parses typed URLs after a pause, shows invalid URLs inline and mirrors field edits', async () => {
  vi.mocked(parseConnectionUrl).mockRejectedValue(
    new Error('malformed connection URL: invalid port number'),
  );
  const { user } = mount();
  const url = screen.getByRole('textbox', { name: 'URL' });
  expect(url).toHaveValue('postgres://localhost:5432');
  await user.clear(url);
  await user.type(url, 'postgres://h:x');
  expect(
    await screen.findByText('malformed connection URL: invalid port number'),
  ).toBeVisible();
  expect(parseConnectionUrl).toHaveBeenCalledTimes(1);
  await user.clear(screen.getByRole('textbox', { name: 'Host' }));
  await user.type(screen.getByRole('textbox', { name: 'Host' }), 'db.test');
  await user.type(screen.getByRole('textbox', { name: 'User' }), 'app');
  await user.type(screen.getByLabelText('Password'), 'pw');
  expect(url).toHaveValue('postgres://app:••••@db.test:5432?sslmode=prefer');
  expect(screen.queryByText(/malformed/)).toBeNull();
});

it('reveals a typed password only on request and blocks copying it while masked', async () => {
  const { user } = mount();
  const password = screen.getByLabelText('Password');
  const reveal = screen.getByRole('button', { name: 'Show password' });
  expect(reveal).toBeDisabled();
  await user.type(password, 'pw-synthetic');
  expect(fireEvent.copy(password)).toBe(false);
  await user.click(reveal);
  expect(password).toHaveAttribute('type', 'text');
  await user.click(screen.getByRole('button', { name: 'Hide password' }));
  expect(password).toHaveAttribute('type', 'password');
});

it('closes on Escape and saves on Return like a native sheet', async () => {
  vi.mocked(createProfile).mockResolvedValue(profile('quick'));
  const { user, handlers } = mount();
  await user.type(screen.getByRole('textbox', { name: 'Name' }), 'quick');
  await user.type(screen.getByRole('textbox', { name: 'User' }), 'app{Enter}');
  expect(createProfile).toHaveBeenCalledOnce();
  await user.keyboard('{Escape}');
  expect(handlers.onClose).toHaveBeenCalledOnce();
});

it('validates required fields before saving or testing', async () => {
  const { user } = mount();
  await user.clear(screen.getByRole('textbox', { name: 'Host' }));
  await user.clear(screen.getByRole('textbox', { name: 'Port' }));
  await user.type(screen.getByRole('textbox', { name: 'Port' }), '70000');
  await user.click(screen.getByRole('button', { name: 'Save' }));
  expect(screen.getByText('Name is required')).toBeVisible();
  expect(screen.getByText('Host is required')).toBeVisible();
  expect(screen.getByText('Port must be 1–65535')).toBeVisible();
  expect(screen.getByText('User is required')).toBeVisible();
  await user.click(screen.getByRole('button', { name: 'Test' }));
  expect(createProfile).not.toHaveBeenCalled();
  expect(testConnection).not.toHaveBeenCalled();
});

it('turns on confirm writes for prod and requires a custom label', async () => {
  const { user } = mount();
  const confirm = screen.getByRole('switch', { name: 'Confirm writes' });
  expect(confirm).toHaveAttribute('aria-checked', 'false');
  await user.click(screen.getByRole('radio', { name: 'Prod' }));
  expect(confirm).toHaveAttribute('aria-checked', 'true');
  await user.click(screen.getByRole('radio', { name: 'Custom' }));
  await user.click(screen.getByRole('button', { name: 'Save' }));
  expect(screen.getByText('Label is required')).toBeVisible();
});

it('tests an existing profile with its stored password and shows version or error', async () => {
  const existing = profile('orders', { hasPassword: true });
  vi.mocked(testConnection)
    .mockResolvedValueOnce({
      steps: [],
      server: server('17.2'),
      error: null,
      elapsedMs: 12,
    })
    .mockRejectedValueOnce(new Error('authentication failed: bad password'));
  const { user } = mount(existing);
  expect(screen.getByPlaceholderText('Saved in Keychain')).toHaveValue('');
  expect(screen.getByRole('textbox', { name: 'URL' })).toHaveValue(
    'postgres://app:••••@localhost:5432/orders',
  );
  await user.click(screen.getByRole('button', { name: 'Test' }));
  expect(testConnection).toHaveBeenCalledWith(
    'id-orders',
    expect.objectContaining({ password: { kind: 'keep' } }),
  );
  expect(
    await screen.findByText('Connected · PostgreSQL 17.2 · 12 ms'),
  ).toBeVisible();
  await user.click(screen.getByRole('button', { name: 'Test' }));
  expect(await screen.findByRole('alert')).toHaveTextContent(
    'authentication failed: bad password',
  );
  vi.mocked(updateProfile).mockRejectedValueOnce(new Error('disk full'));
  await user.click(
    screen.getByRole('button', { name: 'Forget saved password' }),
  );
  await user.click(screen.getByRole('button', { name: 'Save' }));
  expect(updateProfile).toHaveBeenCalledWith(
    'id-orders',
    expect.objectContaining({ password: { kind: 'clear' } }),
  );
  expect(await screen.findByText('disk full')).toBeVisible();
});

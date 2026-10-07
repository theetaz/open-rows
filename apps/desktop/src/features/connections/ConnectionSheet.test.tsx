// SPDX-License-Identifier: Apache-2.0
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import {
  createProfile,
  parsePostgresUrl,
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

it('fills the form from a pasted URL and saves its password only as a fromUrl input', async () => {
  vi.mocked(parsePostgresUrl).mockResolvedValue({
    host: 'db.test',
    port: 6543,
    database: 'orders',
    user: 'app',
    tlsMode: 'prefer',
    passwordProvided: true,
  });
  const saved = profile('orders');
  vi.mocked(createProfile).mockResolvedValue(saved);
  const { user, handlers } = mount();
  const url = screen.getByRole('textbox', { name: 'Paste connection URL' });
  await user.type(url, `${URL_WITH_SECRET}{Enter}`);
  expect(parsePostgresUrl).toHaveBeenCalledWith(URL_WITH_SECRET);
  expect(url).toHaveValue('');
  expect(screen.getByRole('textbox', { name: 'Host' })).toHaveValue('db.test');
  expect(screen.getByRole('textbox', { name: 'Port' })).toHaveValue('6543');
  expect(screen.getByRole('textbox', { name: 'Name' })).toHaveValue('orders');
  expect(screen.getByLabelText('Password')).toHaveValue('');
  expect(screen.getByText(/Password provided in the URL/)).toBeVisible();
  await user.click(screen.getByRole('button', { name: 'Save' }));
  expect(createProfile).toHaveBeenCalledWith(
    expect.objectContaining({
      host: 'db.test',
      port: 6543,
      user: 'app',
      password: { kind: 'fromUrl', url: URL_WITH_SECRET },
    }),
  );
  expect(handlers.onSaved).toHaveBeenCalledWith(saved);
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
    .mockResolvedValueOnce({ server: server('17.2'), elapsedMs: 12 })
    .mockRejectedValueOnce(new Error('authentication failed: bad password'));
  const { user } = mount(existing);
  expect(screen.getByText(/Saved in the keychain/)).toBeVisible();
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

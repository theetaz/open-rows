// SPDX-License-Identifier: Apache-2.0
import { fireEvent, screen } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import {
  createProfile,
  parseConnectionUrl,
  testConnection,
  updateProfile,
} from '@openrows/ipc';
import { profile, server } from '../../../test/ipcStub';
import { continueButton, mountFlow, openStep } from './flowTestKit';

vi.mock('@openrows/ipc', async () =>
  (await import('../../../test/ipcStub')).ipcStub(),
);

const URL_WITH_SECRET = 'postgres://app:pw-synthetic@db.test:6543/orders';

const PARSED = {
  host: 'db.test',
  port: 6543,
  database: 'orders',
  user: 'app',
  tlsMode: 'prefer' as const,
  passwordProvided: false,
  mongodb: null,
  engine: 'postgres' as const,
  file: null,
};

const connected = (elapsedMs: number) => ({
  steps: [],
  server: server('17.2'),
  error: null,
  certificate: null,
  latency: null,
  elapsedMs,
});

it('fills the fields from a pasted URL at once, masks its password and saves it as typed', async () => {
  vi.mocked(parseConnectionUrl).mockResolvedValue(PARSED);
  const saved = profile('orders');
  vi.mocked(createProfile).mockResolvedValue(saved);
  const { user, onDone } = await mountFlow({ engine: 'postgres' });
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
  await user.click(continueButton());
  await user.click(continueButton());
  await user.click(continueButton());
  await user.click(screen.getByRole('button', { name: 'Save' }));
  expect(createProfile).toHaveBeenCalledWith(
    expect.objectContaining({
      host: 'db.test',
      port: 6543,
      user: 'app',
      password: { kind: 'set', value: 'pw-synthetic' },
    }),
  );
  expect(onDone).toHaveBeenCalledWith(saved.id);
});

it('parses typed URLs after a pause, shows invalid URLs inline and mirrors field edits', async () => {
  vi.mocked(parseConnectionUrl).mockRejectedValue(
    new Error('malformed connection URL: invalid port number'),
  );
  const { user } = await mountFlow({ engine: 'postgres' });
  const url = screen.getByRole('textbox', { name: 'URL' });
  expect(url).toHaveValue('postgres://localhost:5432');
  await user.clear(url);
  await user.type(url, 'postgres://h:x');
  expect(
    await screen.findByText('malformed connection URL: invalid port number'),
  ).toBeVisible();
  expect(screen.getByText('Not a valid URL yet')).toBeVisible();
  expect(parseConnectionUrl).toHaveBeenCalledTimes(1);
  await user.clear(screen.getByRole('textbox', { name: 'Host' }));
  await user.type(screen.getByRole('textbox', { name: 'Host' }), 'db.test');
  await user.type(screen.getByRole('textbox', { name: 'User' }), 'app');
  await user.type(screen.getByLabelText('Password'), 'pw');
  expect(url).toHaveValue('postgres://app:••••@db.test:5432?sslmode=prefer');
  expect(screen.queryByText(/malformed/)).toBeNull();
  expect(screen.getByText('In sync')).toBeVisible();
});

it('reveals a typed password only on request, in its field and in the URL, and blocks copying it while masked', async () => {
  const { user } = await mountFlow({ engine: 'postgres' });
  const password = screen.getByLabelText('Password');
  const reveal = screen.getByRole('button', { name: 'Show password' });
  const urlReveal = screen.getByRole('button', {
    name: 'Show password in URL',
  });
  expect(reveal).toBeDisabled();
  expect(urlReveal).toBeDisabled();
  await user.type(screen.getByRole('textbox', { name: 'User' }), 'app');
  await user.type(password, 'pw-synthetic');
  expect(fireEvent.copy(password)).toBe(false);
  const url = screen.getByRole<HTMLInputElement>('textbox', { name: 'URL' });
  expect(url.value).not.toContain('pw-synthetic');
  await user.click(urlReveal);
  expect(url.value).toContain('app:pw-synthetic@');
  await user.click(
    screen.getByRole('button', { name: 'Hide password in URL' }),
  );
  expect(url.value).toContain('app:••••@');
  await user.click(reveal);
  expect(password).toHaveAttribute('type', 'text');
  await user.click(screen.getByRole('button', { name: 'Hide password' }));
  expect(password).toHaveAttribute('type', 'password');
});

it('continues on Return step by step, names the connection after its host, and cancels', async () => {
  vi.mocked(createProfile).mockResolvedValue(profile('quick'));
  const { user, onCancel } = await mountFlow({ engine: 'postgres' });
  await user.type(screen.getByRole('textbox', { name: 'User' }), 'app{Enter}');
  expect(screen.getByRole('heading', { name: 'Security' })).toBeVisible();
  await user.click(continueButton());
  await user.click(continueButton());
  await user.click(screen.getByRole('button', { name: 'Save' }));
  expect(createProfile).toHaveBeenCalledWith(
    expect.objectContaining({ name: 'localhost', user: 'app' }),
  );
  await user.click(screen.getByRole('button', { name: 'Cancel' }));
  expect(onCancel).toHaveBeenCalledOnce();
});

it('validates a step in place before continuing and never saves or tests', async () => {
  const { user } = await mountFlow({ engine: 'postgres' });
  await user.clear(screen.getByRole('textbox', { name: 'Host' }));
  await user.clear(screen.getByRole('textbox', { name: 'Port' }));
  await user.type(screen.getByRole('textbox', { name: 'Port' }), '70000');
  await user.click(continueButton());
  expect(screen.getByText('Host is required')).toBeVisible();
  expect(screen.getByText('Port must be 1–65535')).toBeVisible();
  expect(screen.getByText('User is required')).toBeVisible();
  expect(screen.queryByRole('heading', { name: 'Security' })).toBeNull();
  expect(screen.getByRole('button', { name: /Security/ })).toBeDisabled();
  expect(createProfile).not.toHaveBeenCalled();
  expect(testConnection).not.toHaveBeenCalled();
});

it('turns on confirm writes for prod and requires a custom label', async () => {
  const { user } = await mountFlow({ engine: 'postgres' });
  await user.type(screen.getByRole('textbox', { name: 'User' }), 'app');
  await user.click(continueButton());
  await user.click(continueButton());
  expect(screen.getByRole('radio', { name: /^Unrestricted/ })).toBeChecked();
  await user.click(screen.getByRole('radio', { name: 'Prod' }));
  expect(screen.getByRole('radio', { name: /^Confirm writes/ })).toBeChecked();
  expect(screen.getByText(/Red path bar/)).toBeVisible();
  await user.click(screen.getByRole('radio', { name: 'Custom' }));
  await user.click(continueButton());
  expect(screen.getByText('Label is required')).toBeVisible();
});

it('tests an existing profile with its stored password, shows version or error, and saves from any step', async () => {
  const existing = profile('orders', { hasPassword: true });
  vi.mocked(testConnection)
    .mockResolvedValueOnce(connected(12))
    .mockRejectedValueOnce(new Error('authentication failed: bad password'));
  const { user } = await mountFlow({ profile: existing });
  expect(
    screen.getByRole('heading', { name: 'PostgreSQL details' }),
  ).toBeVisible();
  const saved = screen.getByLabelText('Password');
  expect(saved).toHaveAttribute('type', 'password');
  expect(saved).toHaveAttribute('title', 'Saved in Keychain');
  expect(screen.getByRole('textbox', { name: 'URL' })).toHaveValue(
    'postgres://app:••••@localhost:5432/orders',
  );
  await openStep(user, 'Test and save');
  await user.click(screen.getByRole('button', { name: 'Run test' }));
  expect(testConnection).toHaveBeenCalledWith(
    'id-orders',
    expect.objectContaining({ password: { kind: 'keep' } }),
  );
  expect(
    await screen.findByText('Connected · PostgreSQL 17.2 · 12 ms'),
  ).toBeVisible();
  await user.click(screen.getByRole('button', { name: 'Run again' }));
  expect(await screen.findByRole('alert')).toHaveTextContent(
    'authentication failed: bad password',
  );
  vi.mocked(updateProfile).mockRejectedValueOnce(new Error('disk full'));
  await openStep(user, 'Details');
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

it('saves the statement and idle timeouts from Advanced and sums them up', async () => {
  const saved = profile('orders');
  vi.mocked(updateProfile).mockResolvedValue(saved);
  const { user, onDone } = await mountFlow({ profile: saved });
  await openStep(user, 'Options');
  await user.selectOptions(
    screen.getByRole('combobox', { name: 'Statement timeout' }),
    '60',
  );
  await user.selectOptions(
    screen.getByRole('combobox', { name: 'Idle timeout' }),
    '15',
  );
  expect(
    screen.getByRole('complementary', { name: 'Summary' }),
  ).toHaveTextContent('Timeout 1 min · Idle 15 min');
  await user.click(screen.getByRole('button', { name: 'Save' }));
  expect(updateProfile).toHaveBeenCalledWith(
    'id-orders',
    expect.objectContaining({
      limits: { statementTimeoutSeconds: 60, idleTimeoutMinutes: 15 },
    }),
  );
  expect(onDone).toHaveBeenCalledWith('id-orders');
});

it('saves startup commands from Advanced', async () => {
  vi.mocked(updateProfile).mockResolvedValue(profile('orders'));
  const { user } = await mountFlow({ profile: profile('orders') });
  await openStep(user, 'Options');
  const commands = screen.getByRole('textbox', { name: 'Startup commands' });
  await user.type(commands, 'SET search_path TO app;');
  await user.click(screen.getByRole('button', { name: 'Save' }));
  expect(updateProfile).toHaveBeenCalledWith(
    'id-orders',
    expect.objectContaining({ startupCommands: 'SET search_path TO app;' }),
  );
});

it('saves DuckDB memory profiles and explains irreversible data loss before connecting', async () => {
  const saved = profile('analytics', {
    engine: 'duckdb',
    file: { path: '', memory: true, readOnly: false, create: false },
  });
  vi.mocked(updateProfile).mockResolvedValue(saved);
  const { user } = await mountFlow({ profile: saved });
  expect(
    screen.getByText(/losing the driver process permanently loses all data/),
  ).toBeVisible();
  expect(
    screen.queryByRole('textbox', { name: 'Database file' }),
  ).not.toBeInTheDocument();
  expect(
    screen.getByText(
      /Import, backup, running SQL files and explain visualization are not\s+available yet/,
    ),
  ).toBeVisible();
  expect(screen.queryByRole('button', { name: /Security/ })).toBeNull();
  await user.click(screen.getByRole('button', { name: 'Save' }));
  expect(updateProfile).toHaveBeenCalledWith(
    saved.id,
    expect.objectContaining({
      engine: 'duckdb',
      file: { path: '', memory: true, readOnly: false, create: false },
    }),
  );
});

it('opens a safety change at Options with the policy focused', async () => {
  await mountFlow({ profile: profile('orders'), focus: 'safety' });
  expect(screen.getByRole('radio', { name: /^Unrestricted/ })).toHaveFocus();
});

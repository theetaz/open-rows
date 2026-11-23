// SPDX-License-Identifier: Apache-2.0
import { screen, waitFor } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import {
  createProfile,
  parseConnectionUrl,
  testConnection,
} from '@openrows/ipc';
import { profile, server } from '../../../test/ipcStub';
import { continueButton, mountFlow } from './flowTestKit';

vi.mock('@openrows/ipc', async () =>
  (await import('../../../test/ipcStub')).ipcStub(),
);

it('picks an engine tile, refuses types without a driver and searches', async () => {
  const { user } = await mountFlow();
  expect(continueButton()).toBeDisabled();
  expect(
    screen.getByRole('button', { name: /^Oracle.*Coming soon/ }),
  ).toHaveAttribute('aria-disabled', 'true');
  await user.click(screen.getByRole('button', { name: /^Oracle/ }));
  expect(
    screen.getByRole('heading', { level: 2, name: 'New connection' }),
  ).toBeVisible();
  expect(
    screen.getByRole('button', { name: /^SQL Server.*Beta/ }),
  ).toBeVisible();
  await user.type(
    screen.getByRole('searchbox', { name: 'Search engines' }),
    'maria',
  );
  expect(screen.queryByRole('button', { name: /^PostgreSQL/ })).toBeNull();
  await user.type(
    screen.getByRole('searchbox', { name: 'Search engines' }),
    'zzz',
  );
  expect(screen.getByText('No connection type matches.')).toBeVisible();
  await user.clear(screen.getByRole('searchbox', { name: 'Search engines' }));
  await user.click(screen.getByRole('button', { name: /^MySQL/ }));
  expect(screen.getByRole('heading', { name: 'MySQL details' })).toBeVisible();
  expect(screen.getByRole('textbox', { name: 'Port' })).toHaveValue('3306');
  expect(screen.getByRole('textbox', { name: 'URL' })).toHaveValue(
    'mysql://localhost:3306',
  );
});

it('a SQLite engine swaps the server for a file and skips Security', async () => {
  const { user } = await mountFlow();
  await user.click(screen.getByRole('button', { name: /^SQLite/ }));
  expect(screen.getByRole('heading', { name: 'SQLite details' })).toBeVisible();
  expect(screen.queryByRole('textbox', { name: 'Host' })).toBeNull();
  expect(screen.queryByRole('button', { name: /Security/ })).toBeNull();
  expect(screen.getByText('Step 2 of 4')).toBeVisible();
});

it('a pasted URL names the engine, fills every field, guesses the environment and runs the test', async () => {
  vi.mocked(parseConnectionUrl).mockResolvedValue({
    engine: 'postgres',
    file: null,
    host: 'orders-db.prod.internal',
    port: 5432,
    database: 'orders_prod',
    user: 'app_rw',
    tlsMode: 'verify-full',
    passwordProvided: false,
    mongodb: null,
  });
  vi.mocked(testConnection).mockResolvedValue({
    steps: [
      {
        kind: 'dns',
        label: 'Resolve orders-db.prod.internal',
        status: 'ok',
        detail: null,
        hint: null,
        elapsedMs: 14,
      },
    ],
    server: server('17.2'),
    error: null,
    certificate: null,
    latency: null,
    elapsedMs: 126,
  });
  vi.mocked(createProfile).mockResolvedValue(profile('orders_prod'));
  const { user, onDone } = await mountFlow();
  const paste = screen.getByRole('textbox', { name: 'Paste a connection URL' });
  expect(paste).toHaveFocus();
  await user.paste(
    'postgres://app_rw:pw-synthetic@orders-db.prod.internal/orders_prod',
  );
  expect(paste).toHaveValue(
    'postgres://app_rw:••••@orders-db.prod.internal/orders_prod',
  );
  expect(parseConnectionUrl).toHaveBeenLastCalledWith(
    'postgres://app_rw@orders-db.prod.internal/orders_prod',
  );
  expect(
    await screen.findByText(/Detected PostgreSQL at orders-db/),
  ).toBeVisible();
  await user.keyboard('{Enter}');
  expect(screen.getByRole('heading', { name: 'Test and save' })).toBeVisible();
  await waitFor(() => {
    expect(testConnection).toHaveBeenCalledWith(
      null,
      expect.objectContaining({
        host: 'orders-db.prod.internal',
        password: { kind: 'set', value: 'pw-synthetic' },
      }),
    );
  });
  expect(
    await screen.findByText('Resolve orders-db.prod.internal'),
  ).toBeVisible();
  expect(screen.getByText('14 ms')).toBeVisible();
  await user.click(screen.getByRole('button', { name: 'Save' }));
  const [input] = vi.mocked(createProfile).mock.calls.at(-1) ?? [];
  expect(input?.name).toBe('orders_prod');
  expect(input?.env.kind).toBe('prod');
  expect(input?.safety).toBe('confirmWrites');
  expect(onDone).toHaveBeenCalledWith('id-orders_prod');
  expect(document.body.innerHTML).not.toContain('pw-synthetic');
});

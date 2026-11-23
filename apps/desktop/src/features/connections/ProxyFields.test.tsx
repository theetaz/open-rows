// SPDX-License-Identifier: Apache-2.0
import { screen, within } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import { createProfile, testConnection } from '@openrows/ipc';
import type { TestReport } from '@openrows/ipc';
import { profile, server } from '../../test/ipcStub';
import {
  continueButton,
  mountFlow,
  runTest,
  saveFlow,
  summary,
} from './flow/flowTestKit';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);

async function mount(existing?: ReturnType<typeof profile>) {
  const { user } = await mountFlow(
    existing ? { profile: existing } : { engine: 'postgres' },
  );
  return user;
}

it('saves an HTTP proxy with a user and its password for the keychain', async () => {
  vi.mocked(createProfile).mockResolvedValue(profile('proxied'));
  const user = await mount();
  await user.type(screen.getByRole('textbox', { name: 'Name' }), 'proxied');
  await user.type(screen.getByRole('textbox', { name: 'User' }), 'app');
  await user.click(continueButton());
  await user.click(
    screen.getByRole('switch', { name: 'Connect through a proxy' }),
  );
  await user.selectOptions(
    screen.getByRole('combobox', { name: 'Proxy type' }),
    'http',
  );
  await user.type(
    screen.getByRole('textbox', { name: 'Proxy host' }),
    'proxy.corp',
  );
  await user.type(screen.getByRole('textbox', { name: 'Proxy port' }), '3128');
  await user.type(screen.getByRole('textbox', { name: 'Proxy user' }), 'ops');
  await user.type(screen.getByLabelText('Proxy password'), 'pw-synthetic');
  expect(summary()).toHaveTextContent('HTTP ops@proxy.corp:3128');
  await saveFlow(user);
  expect(createProfile).toHaveBeenCalledWith(
    expect.objectContaining({
      proxy: {
        kind: 'http',
        host: 'proxy.corp',
        port: 3128,
        user: 'ops',
        password: { kind: 'set', value: 'pw-synthetic' },
      },
    }),
  );
});

it('offers bundled cloud CAs instead of a CA file and accepts them for Verify CA', async () => {
  vi.mocked(createProfile).mockResolvedValue(profile('rds'));
  const user = await mount();
  await user.type(screen.getByRole('textbox', { name: 'Name' }), 'rds');
  await user.type(screen.getByRole('textbox', { name: 'User' }), 'app');
  await user.click(continueButton());
  await user.selectOptions(
    screen.getByRole('combobox', { name: 'TLS mode' }),
    'verify-ca',
  );
  expect(screen.getByRole('button', { name: 'Choose ca file' })).toBeVisible();
  await user.selectOptions(
    screen.getByRole('combobox', { name: 'Cloud provider CA' }),
    'awsRds',
  );
  expect(screen.queryByRole('button', { name: 'Choose ca file' })).toBeNull();
  await saveFlow(user);
  const [input] = vi.mocked(createProfile).mock.calls.at(-1) ?? [];
  expect(input?.tlsMode).toBe('verify-ca');
  expect(input?.tlsFiles).toMatchObject({ caFile: null, cloudCa: 'awsRds' });
});

const report: TestReport = {
  steps: [
    {
      kind: 'tls',
      label: 'TLS handshake',
      status: 'warning',
      detail: 'Certificate expires in 9 days · TLS 1.3',
      hint: null,
      elapsedMs: 4,
    },
    {
      kind: 'latency',
      label: 'Round trip (SELECT 1)',
      status: 'ok',
      detail: 'median 2.4 ms · min 1.9 ms · max 3.1 ms over 3 queries',
      hint: null,
      elapsedMs: 2,
    },
  ],
  server: server('17.2'),
  error: null,
  certificate: {
    subject: 'CN=db.example.com',
    issuer: 'CN=Example CA',
    notBefore: '2026-01-01T00:00:00Z',
    notAfter: '2026-10-18T00:00:00Z',
    daysLeft: 9,
    expiringSoon: true,
    altNames: ['db.example.com', '10.0.0.5'],
    verified: true,
    hostVerified: true,
  },
  latency: { medianMs: 2.4, minMs: 1.9, maxMs: 3.1, samples: 3 },
  elapsedMs: 30,
};

it('shows the certificate with its expiry warning and the latency', async () => {
  vi.mocked(testConnection).mockResolvedValueOnce(report);
  const user = await mount(profile('db'));
  await runTest(user);
  expect(await screen.findByText('Latency 2.4 ms · fast')).toBeVisible();
  const certificate = screen.getByRole('region', {
    name: 'Server certificate',
  });
  expect(certificate).toHaveTextContent('CN=db.example.com');
  expect(certificate).toHaveTextContent('CN=Example CA');
  expect(certificate).toHaveTextContent('db.example.com, 10.0.0.5');
  expect(within(certificate).getByText('expires in 9 days')).toBeVisible();
  expect(screen.getByText('Round trip (SELECT 1)')).toBeVisible();
});

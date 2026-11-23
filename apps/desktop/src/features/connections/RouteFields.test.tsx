// SPDX-License-Identifier: Apache-2.0
import { screen, within } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import {
  createProfile,
  pickConnectionFile,
  testConnection,
  updateProfile,
} from '@openrows/ipc';
import type { TestReport } from '@openrows/ipc';
import { profile, server } from '../../test/ipcStub';
import {
  continueButton,
  mountFlow,
  openStep,
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

it('reveals the SSH rows only when switched on, then saves a key-file tunnel with its passphrase', async () => {
  vi.mocked(createProfile).mockResolvedValue(profile('tunnelled'));
  vi.mocked(pickConnectionFile).mockResolvedValueOnce(
    '/Users/me/.ssh/id_ed25519',
  );
  const user = await mount();
  await user.type(screen.getByRole('textbox', { name: 'Name' }), 'tunnelled');
  await user.type(screen.getByRole('textbox', { name: 'User' }), 'app');
  await user.click(continueButton());
  expect(screen.queryByRole('textbox', { name: 'SSH host' })).toBeNull();
  await user.click(screen.getByRole('switch', { name: 'Connect over SSH' }));
  await user.type(screen.getByRole('textbox', { name: 'SSH host' }), 'bastion');
  await user.type(screen.getByRole('textbox', { name: 'SSH user' }), 'ops');
  await user.selectOptions(
    screen.getByRole('combobox', { name: 'SSH authentication' }),
    'privateKey',
  );
  await user.click(screen.getByRole('button', { name: 'Choose key file' }));
  expect(pickConnectionFile).toHaveBeenCalledWith('sshKey');
  expect(
    await screen.findByDisplayValue('/Users/me/.ssh/id_ed25519'),
  ).toBeVisible();
  await user.type(screen.getByLabelText('Passphrase'), 'pp-synthetic');
  expect(screen.getByLabelText('Passphrase')).toHaveAttribute(
    'type',
    'password',
  );
  expect(summary()).toHaveTextContent('SSH ops@bastion');
  await saveFlow(user);
  expect(createProfile).toHaveBeenCalledWith(
    expect.objectContaining({
      ssh: {
        host: 'bastion',
        port: null,
        user: 'ops',
        auth: { method: 'privateKey', keyFile: '/Users/me/.ssh/id_ed25519' },
        jumpHosts: [],
        password: { kind: 'keep' },
        passphrase: { kind: 'set', value: 'pp-synthetic' },
      },
    }),
  );
});

it('keeps Continue on Security while SSH or TLS fields are invalid', async () => {
  const user = await mount();
  await user.type(screen.getByRole('textbox', { name: 'User' }), 'app');
  await user.click(continueButton());
  await user.click(screen.getByRole('switch', { name: 'Connect over SSH' }));
  await user.selectOptions(
    screen.getByRole('combobox', { name: 'TLS mode' }),
    'verify-ca',
  );
  await user.click(continueButton());
  expect(screen.getByRole('heading', { name: 'Security' })).toBeVisible();
  expect(screen.getByText('SSH host is required')).toBeVisible();
  expect(screen.getByText('Verify CA needs a CA certificate')).toBeVisible();
  expect(createProfile).not.toHaveBeenCalled();
});

it('flags unverified TLS to a remote host until a verify mode is chosen', async () => {
  const user = await mount();
  const host = screen.getByRole('textbox', { name: 'Host' });
  await user.clear(host);
  await user.type(host, 'db.example.com');
  await user.type(screen.getByRole('textbox', { name: 'User' }), 'app');
  await user.click(continueButton());
  expect(screen.getByText('Certificate not verified')).toBeVisible();
  await user.selectOptions(
    screen.getByRole('combobox', { name: 'TLS mode' }),
    'verify-full',
  );
  expect(screen.queryByText('Certificate not verified')).toBeNull();
  expect(summary()).toHaveTextContent('Verify full');
});

it('edits a saved tunnel without reading its stored SSH password', async () => {
  vi.mocked(updateProfile).mockResolvedValue(profile('db'));
  const saved = profile('db', {
    ssh: {
      host: 'bastion',
      port: 2222,
      user: 'ops',
      auth: { method: 'password' },
      jumpHosts: [],
      hasPassword: true,
      hasPassphrase: false,
    },
  });
  const user = await mount(saved);
  expect(summary()).toHaveTextContent('SSH ops@bastion:2222');
  await openStep(user, 'Security');
  expect(
    screen.getByRole('switch', { name: 'Connect over SSH' }),
  ).toBeChecked();
  await user.click(
    screen.getByRole('button', { name: 'Forget saved SSH password' }),
  );
  await saveFlow(user);
  const [id, input] = vi.mocked(updateProfile).mock.calls[0] ?? [];
  expect(id).toBe('id-db');
  expect(input?.ssh?.password).toEqual({ kind: 'clear' });
});

const failedReport: TestReport = {
  steps: [
    {
      kind: 'sshConnect',
      label: 'Connect to SSH server bastion:22',
      status: 'ok',
      detail: null,
      hint: null,
      elapsedMs: 14,
    },
    {
      kind: 'sshAuth',
      label: 'SSH login as ops@bastion',
      status: 'failed',
      detail: 'SSH: authentication failed for ops@bastion (tried: password)',
      hint: 'Check the SSH user and authentication method.',
      elapsedMs: null,
    },
  ],
  server: null,
  error: { kind: 'ssh', message: 'SSH: authentication failed', code: null },
  certificate: null,
  latency: null,
  elapsedMs: 52,
};

it('shows Test connection step by step with timings, the failing step and its hint', async () => {
  vi.mocked(testConnection).mockResolvedValueOnce(failedReport);
  const user = await mount(profile('db'));
  await runTest(user);
  const report = await screen.findByRole('alert');
  expect(report).toHaveTextContent('Connection failed · 52 ms');
  const steps = within(report).getAllByRole('listitem');
  expect(steps).toHaveLength(2);
  expect(steps[0]).toHaveTextContent('Connect to SSH server bastion:22');
  expect(steps[0]).toHaveTextContent('14 ms');
  expect(steps[1]).toHaveTextContent('failed');
  expect(steps[1]).toHaveTextContent('Check the SSH user');
  vi.mocked(testConnection).mockResolvedValueOnce({
    steps: [],
    server: server('17.2'),
    error: null,
    certificate: null,
    latency: null,
    elapsedMs: 9,
  });
  await runTest(user);
  expect(
    await screen.findByText('Connected · PostgreSQL 17.2 · 9 ms'),
  ).toBeVisible();
});

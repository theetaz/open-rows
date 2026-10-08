// SPDX-License-Identifier: Apache-2.0
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import {
  createProfile,
  pickConnectionFile,
  testConnection,
  updateProfile,
} from '@openrows/ipc';
import type { TestReport } from '@openrows/ipc';
import { ThemeProvider } from '@openrows/ui';
import { profile, server } from '../../test/ipcStub';
import { ConnectionSheet } from './ConnectionSheet';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);

function mount(existing?: ReturnType<typeof profile>) {
  render(
    <ThemeProvider>
      <ConnectionSheet
        profile={existing}
        onClose={vi.fn()}
        onSaved={vi.fn()}
        onDelete={vi.fn()}
        onDuplicate={vi.fn()}
      />
    </ThemeProvider>,
  );
  return userEvent.setup();
}

it('keeps SSH collapsed until asked, then saves a key-file tunnel with its passphrase', async () => {
  vi.mocked(createProfile).mockResolvedValue(profile('tunnelled'));
  vi.mocked(pickConnectionFile).mockResolvedValueOnce(
    '/Users/me/.ssh/id_ed25519',
  );
  const user = mount();
  await user.type(screen.getByRole('textbox', { name: 'Name' }), 'tunnelled');
  await user.type(screen.getByRole('textbox', { name: 'User' }), 'app');
  const section = screen.getByRole('button', { name: 'SSH · Off' });
  expect(section).toHaveAttribute('aria-expanded', 'false');
  await user.click(section);
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
  expect(
    screen.getByRole('button', { name: 'SSH · ops@bastion' }),
  ).toBeVisible();
  await user.click(screen.getByRole('button', { name: 'Save' }));
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

it('opens a section with invalid fields and blocks saving', async () => {
  const user = mount();
  await user.click(screen.getByRole('button', { name: 'SSH · Off' }));
  await user.click(screen.getByRole('switch', { name: 'Connect over SSH' }));
  const section = screen.getByRole('button', { name: 'SSH · no host' });
  await user.click(section);
  expect(section).toHaveAttribute('aria-expanded', 'false');
  await user.click(screen.getByRole('button', { name: /^TLS/ }));
  await user.selectOptions(
    screen.getByRole('combobox', { name: 'TLS mode' }),
    'verify-ca',
  );
  await user.click(screen.getByRole('button', { name: 'Save' }));
  expect(section).toHaveAttribute('aria-expanded', 'true');
  expect(screen.getByText('SSH host is required')).toBeVisible();
  expect(screen.getByText('Verify CA needs a CA certificate')).toBeVisible();
  expect(createProfile).not.toHaveBeenCalled();
});

it('flags unverified TLS to a remote host even while the section is collapsed', async () => {
  const user = mount();
  expect(screen.queryByText('Certificate not verified')).toBeNull();
  const host = screen.getByRole('textbox', { name: 'Host' });
  await user.clear(host);
  await user.type(host, 'db.example.com');
  expect(screen.getByText('Certificate not verified')).toBeVisible();
  await user.click(screen.getByRole('button', { name: /^TLS/ }));
  await user.selectOptions(
    screen.getByRole('combobox', { name: 'TLS mode' }),
    'verify-full',
  );
  expect(screen.queryByText('Certificate not verified')).toBeNull();
  expect(
    screen.getByRole('button', { name: 'TLS · Verify full' }),
  ).toBeVisible();
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
  const user = mount(saved);
  const section = screen.getByRole('button', {
    name: 'SSH · ops@bastion:2222',
  });
  expect(section).toHaveAttribute('aria-expanded', 'true');
  await user.click(
    screen.getByRole('button', { name: 'Forget saved SSH password' }),
  );
  await user.click(screen.getByRole('button', { name: 'Save' }));
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
  elapsedMs: 52,
};

it('shows Test connection step by step with timings, the failing step and its hint', async () => {
  vi.mocked(testConnection).mockResolvedValueOnce(failedReport);
  const user = mount(profile('db'));
  await user.click(screen.getByRole('button', { name: 'Test' }));
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
    elapsedMs: 9,
  });
  await user.click(screen.getByRole('button', { name: 'Test' }));
  expect(
    await screen.findByText('Connected · PostgreSQL 17.2 · 9 ms'),
  ).toBeVisible();
});

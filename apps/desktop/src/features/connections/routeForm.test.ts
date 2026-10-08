// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { profile } from '../../test/ipcStub';
import {
  emptySsh,
  emptyTls,
  jumpList,
  sshFromProfile,
  sshInput,
  sshSummary,
  tlsInput,
  tlsWarning,
  validateRoute,
  withSecret,
} from './routeForm';
import type { SshDraft } from './routeForm';

const keyed: SshDraft = {
  ...emptySsh(),
  enabled: true,
  host: ' bastion ',
  port: '2222',
  user: ' ops ',
  auth: 'privateKey',
  keyFile: ' ~/.ssh/id_ed25519 ',
  jumpHosts: 'jump1, ops@jump2:22\n',
};

it('sends no SSH settings while SSH is off and trimmed ones when on', () => {
  expect(sshInput(emptySsh())).toBeNull();
  expect(
    sshInput({ ...keyed, passphrase: withSecret(keyed.passphrase, 'pp') }),
  ).toEqual({
    host: 'bastion',
    port: 2222,
    user: 'ops',
    auth: { method: 'privateKey', keyFile: '~/.ssh/id_ed25519' },
    jumpHosts: ['jump1', 'ops@jump2:22'],
    password: { kind: 'keep' },
    passphrase: { kind: 'set', value: 'pp' },
  });
  expect(jumpList(' , a ,\nb ')).toEqual(['a', 'b']);
});

it('never invents a secret: a cleared field keeps the stored one', () => {
  const stored = { value: '', mode: 'keep' as const, stored: true };
  expect(withSecret(stored, 'x').mode).toBe('set');
  expect(withSecret(withSecret(stored, 'x'), '').mode).toBe('keep');
  expect(tlsInput(emptyTls()).keyPassphrase).toEqual({ kind: 'keep' });
});

it('loads saved SSH settings with stored-secret flags but no values', () => {
  const saved = profile('db', {
    ssh: {
      host: 'bastion',
      port: null,
      user: null,
      auth: { method: 'password' },
      jumpHosts: ['a', 'b'],
      hasPassword: true,
      hasPassphrase: false,
    },
  });
  const ssh = sshFromProfile(saved);
  expect(ssh).toMatchObject({ enabled: true, port: '', jumpHosts: 'a, b' });
  expect(ssh.password).toEqual({ value: '', mode: 'keep', stored: true });
  expect(sshSummary(ssh)).toBe('bastion');
  expect(sshSummary(keyed)).toBe('ops@bastion:2222');
  expect(sshSummary(emptySsh())).toBe('Off');
});

it('validates the SSH and TLS sections', () => {
  const broken: SshDraft = {
    ...keyed,
    host: ' ',
    port: '0',
    keyFile: '',
    jumpHosts: 'bad host',
  };
  expect(validateRoute(broken, emptyTls(), 'prefer')).toEqual({
    sshHost: 'SSH host is required',
    sshPort: 'Port must be 1–65535',
    sshKeyFile: 'Choose a private key file',
    sshJumpHosts: 'Use [user@]host[:port], separated by commas',
  });
  const tls = { ...emptyTls(), clientCert: '/c.pem' };
  expect(validateRoute(emptySsh(), tls, 'verify-ca')).toEqual({
    tlsCaFile: 'Verify CA needs a CA certificate',
    tlsClientKey: 'Choose both a client certificate and its key',
  });
  expect(validateRoute(keyed, emptyTls(), 'verify-full')).toEqual({});
});

it('flags remote connections whose certificate is not verified', () => {
  expect(tlsWarning('localhost', 'disable', '')).toBeNull();
  expect(tlsWarning('/tmp', 'prefer', '')).toBeNull();
  expect(tlsWarning('db.example.com', 'verify-full', '')).toBeNull();
  expect(tlsWarning('db.example.com', 'require', '/ca.pem')).toBeNull();
  expect(tlsWarning('db.example.com', 'require', '')?.badge).toBe(
    'Certificate not verified',
  );
  expect(tlsWarning('db.example.com', 'disable', '')?.badge).toBe(
    'Not encrypted',
  );
});

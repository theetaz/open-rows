// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { profile } from '../../test/ipcStub';
import {
  emptyProxy,
  proxyErrors,
  proxyFromProfile,
  proxyInput,
  proxySummary,
} from './proxyForm';
import { withSecret } from './routeForm';

it('sends nothing while off and the usual port when the port is blank', () => {
  expect(proxyInput(emptyProxy())).toBeNull();
  const socks = { ...emptyProxy(), enabled: true, host: ' proxy.corp ' };
  expect(proxyInput(socks)).toEqual({
    kind: 'socks5',
    host: 'proxy.corp',
    port: 1080,
    user: null,
    password: { kind: 'clear' },
  });
  const http = { ...socks, kind: 'http' as const, port: '3128', user: 'ops' };
  const typed = {
    ...http,
    password: withSecret(http.password, 'pw-synthetic'),
  };
  expect(proxyInput(typed)).toMatchObject({
    kind: 'http',
    port: 3128,
    user: 'ops',
    password: { kind: 'set', value: 'pw-synthetic' },
  });
});

it('requires a host, a valid port and SSH in front of a local socket', () => {
  const on = { ...emptyProxy(), enabled: true };
  expect(proxyErrors(emptyProxy(), 'db', false)).toEqual({});
  expect(proxyErrors(on, 'db', false)).toEqual({
    proxyHost: 'Proxy host is required',
  });
  const named = { ...on, host: 'proxy.corp', port: '70000' };
  expect(proxyErrors(named, 'db', false)).toEqual({
    proxyPort: 'Port must be 1–65535',
  });
  const socket = { ...on, host: 'proxy.corp' };
  expect(proxyErrors(socket, '/var/run/postgresql', false).proxyHost).toBe(
    'A proxy cannot reach a local socket; use SSH',
  );
  expect(proxyErrors(socket, '/var/run/postgresql', true)).toEqual({});
});

it('summarises the section and loads a saved proxy without its password', () => {
  expect(proxySummary(emptyProxy())).toBe('Off');
  const saved = profile('db', {
    proxy: {
      kind: 'http',
      host: 'proxy.corp',
      port: 3128,
      user: 'ops',
      hasPassword: true,
    },
  });
  const draft = proxyFromProfile(saved);
  expect(draft).toMatchObject({ enabled: true, port: '3128', user: 'ops' });
  expect(draft.password).toEqual({ value: '', mode: 'keep', stored: true });
  expect(proxySummary(draft)).toBe('HTTP ops@proxy.corp:3128');
  expect(proxySummary({ ...emptyProxy(), enabled: true })).toBe(
    'SOCKS5 no host:1080',
  );
});

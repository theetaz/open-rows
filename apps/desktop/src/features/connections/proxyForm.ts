// SPDX-License-Identifier: Apache-2.0
import type { ProfileDto, ProxyInput, ProxyKindDto } from '@openrows/ipc';
import { emptySecret, secretInput, validPort } from './routeForm';
import type { SecretDraft } from './routeForm';

/** "Proxy" section values, strings as typed. */
export type ProxyDraft = {
  enabled: boolean;
  kind: ProxyKindDto;
  host: string;
  port: string;
  user: string;
  password: SecretDraft;
};

export type ProxyErrors = Partial<Record<'proxyHost' | 'proxyPort', string>>;

/** The usual port of each proxy type, shown as the placeholder. */
export const PROXY_PORTS: Record<ProxyKindDto, string> = {
  socks5: '1080',
  http: '8080',
};

export function emptyProxy(): ProxyDraft {
  return {
    enabled: false,
    kind: 'socks5',
    host: '',
    port: '',
    user: '',
    password: emptySecret(),
  };
}

export function proxyFromProfile(profile: ProfileDto): ProxyDraft {
  const proxy = profile.proxy;
  if (!proxy) return emptyProxy();
  return {
    enabled: true,
    kind: proxy.kind,
    host: proxy.host,
    port: String(proxy.port),
    user: proxy.user ?? '',
    password: emptySecret(proxy.hasPassword),
  };
}

/** The command payload; `null` when the proxy is off. A blank port uses the type's usual one. */
export function proxyInput(proxy: ProxyDraft): ProxyInput | null {
  if (!proxy.enabled) return null;
  const user = proxy.user.trim();
  return {
    kind: proxy.kind,
    host: proxy.host.trim(),
    port: Number(proxy.port.trim() || PROXY_PORTS[proxy.kind]),
    user: user || null,
    password: user ? secretInput(proxy.password) : { kind: 'clear' },
  };
}

/**
 * Required host, a valid port, and no proxy in front of a local Unix
 * socket unless SSH carries the connection to the socket's server.
 */
export function proxyErrors(
  proxy: ProxyDraft,
  host: string,
  ssh: boolean,
): ProxyErrors {
  if (!proxy.enabled) return {};
  const errors: ProxyErrors = {};
  if (!proxy.host.trim()) errors.proxyHost = 'Proxy host is required';
  else if (host.trim().startsWith('/') && !ssh)
    errors.proxyHost = 'A proxy cannot reach a local socket; use SSH';
  if (proxy.port.trim() && !validPort(proxy.port))
    errors.proxyPort = 'Port must be 1–65535';
  return errors;
}

/** Collapsed section summary: `Off`, or `SOCKS5 · user@host:port`. */
export function proxySummary(proxy: ProxyDraft): string {
  if (!proxy.enabled) return 'Off';
  const kind = proxy.kind === 'socks5' ? 'SOCKS5' : 'HTTP';
  const user = proxy.user.trim() ? `${proxy.user.trim()}@` : '';
  const port = proxy.port.trim() || PROXY_PORTS[proxy.kind];
  return `${kind} ${user}${proxy.host.trim() || 'no host'}:${port}`;
}

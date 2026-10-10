// SPDX-License-Identifier: Apache-2.0
import type { TlsModeDto } from '@openrows/ipc';
import { isFileEngine, urlScheme } from './engines';
import type { Draft } from './form';

/** What the URL field shows in place of any password. */
export const PASSWORD_MASK = '••••';

/** The password found in a typed URL. */
export type UrlPassword =
  | { kind: 'none' }
  | { kind: 'masked' }
  | { kind: 'typed'; value: string }
  | { kind: 'invalid' };

/** A typed URL split for display (`masked`) and for the core parser (`stripped`). */
type SplitUrl = {
  masked: string;
  stripped: string;
  password: UrlPassword;
};

/** Password characters `start..end`; `cut` more characters before it are syntax. */
type Span = { start: number; end: number; cut: number };

/** The userinfo password (`user:secret@`) span, located like libpq/the core parser. */
function userinfoSpan(text: string): Span | undefined {
  const scheme = text.indexOf('://');
  if (scheme < 0) return undefined;
  const from = scheme + 3;
  const rest = text.slice(from);
  const authority = rest.split(/[/?#]/, 1)[0] ?? '';
  const at = authority.lastIndexOf('@');
  const colon = authority.indexOf(':');
  if (at < 0 || colon < 0 || colon > at) return undefined;
  return { start: from + colon + 1, end: from + at, cut: 1 };
}

/** A `password=` query parameter (or JDBC `;password=`) span. */
function querySpan(text: string): Span | undefined {
  const match = /[?&;]password=([^&#;]*)/i.exec(text);
  if (!match) return undefined;
  const end = match.index + match[0].length;
  const start = end - (match[1] ?? '').length;
  return { start, end, cut: 'password='.length };
}

/** Remove the password and its syntax, keeping the query well formed. */
function strip(text: string, span: Span): string {
  const head = text.slice(0, span.start - span.cut);
  const tail = text.slice(span.end);
  if (head.endsWith(';')) return head.slice(0, -1) + tail;
  if (!/[?&]$/.test(head)) return head + tail;
  return tail.startsWith('&') ? head + tail.slice(1) : head.slice(0, -1) + tail;
}

function decode(raw: string): UrlPassword {
  if (raw.includes('•')) return { kind: 'masked' };
  try {
    return { kind: 'typed', value: decodeURIComponent(raw) };
  } catch {
    return { kind: 'invalid' };
  }
}

/**
 * Replace the password with {@link PASSWORD_MASK} for display and remove it
 * for parsing, so the secret is never echoed and never sent to the core
 * parser. A segment that still contains mask dots keeps the current password.
 */
export function splitUrl(text: string): SplitUrl {
  const span = userinfoSpan(text) ?? querySpan(text);
  if (!span || span.end === span.start)
    return { masked: text, stripped: text, password: { kind: 'none' } };
  const before = text.slice(0, span.start);
  const after = text.slice(span.end);
  return {
    masked: `${before}${PASSWORD_MASK}${after}`,
    stripped: strip(text, span),
    password: decode(text.slice(span.start, span.end)),
  };
}

/** libpq default the core applies when `sslmode` is absent. */
function defaultTls(host: string): Draft['tlsMode'] {
  const local =
    ['localhost', '127.0.0.1', '::1'].includes(host) || host.startsWith('/');
  return local ? 'prefer' : 'verify-full';
}

function hasPassword(draft: Draft): boolean {
  if (draft.password) return true;
  return draft.hasStoredPassword && draft.passwordMode === 'keep';
}

/** `MySQL` `ssl-mode` names; `allow` has none and keeps the libpq spelling. */
const MYSQL_SSL: Partial<Record<TlsModeDto, string>> = {
  disable: 'DISABLED',
  prefer: 'PREFERRED',
  require: 'REQUIRED',
  'verify-ca': 'VERIFY_CA',
  'verify-full': 'VERIFY_IDENTITY',
};

/** SQL Server's `encrypt` and `trustServerCertificate` for a TLS mode. */
const SQL_SERVER_TLS: Partial<Record<TlsModeDto, string>> = {
  allow: 'encrypt=false',
  require: 'encrypt=true&trustServerCertificate=true',
  'verify-full': 'encrypt=true&trustServerCertificate=false',
};

function tlsQuery(draft: Draft): string {
  if (draft.engine === 'clickhouse')
    return ['disable', 'verify-full'].includes(draft.tlsMode)
      ? ''
      : `?sslmode=${draft.tlsMode}`;
  if (draft.tlsMode === defaultTls(draft.host)) return '';
  const sqlServer =
    draft.engine === 'sqlserver' ? SQL_SERVER_TLS[draft.tlsMode] : undefined;
  if (sqlServer) return `?${sqlServer}`;
  const mysql = draft.engine === 'mysql' || draft.engine === 'mariadb';
  const name = mysql ? MYSQL_SSL[draft.tlsMode] : undefined;
  return name ? `?ssl-mode=${name}` : `?sslmode=${draft.tlsMode}`;
}

/** `sqlite:/path`, `?mode=ro` when the file opens read-only; `sqlite::memory:` in memory. */
function fileUrl(draft: Draft): string {
  if (draft.fileMemory) return `${urlScheme(draft.engine)}::memory:`;
  const path = encodeURI(draft.filePath.trim());
  return `${urlScheme(draft.engine)}:${path}${draft.fileReadOnly ? '?mode=ro' : ''}`;
}

/** The URL for the current fields, with any password shown only as the mask. */
export function composeUrl(draft: Draft): string {
  if (isFileEngine(draft.engine)) return fileUrl(draft);
  if (draft.engine === 'libsql') {
    const scheme = draft.tlsMode === 'disable' ? 'http' : 'libsql';
    const host = draft.host.includes(':') ? `[${draft.host}]` : draft.host;
    return `${scheme}://${host.trim()}:${draft.port || (scheme === 'http' ? '80' : '443')}`;
  }
  const user = encodeURIComponent(draft.user.trim());
  const secret = hasPassword(draft) ? `:${PASSWORD_MASK}` : '';
  const auth = user || secret ? `${user}${secret}@` : '';
  const host = draft.host.includes(':') ? `[${draft.host}]` : draft.host;
  const port = draft.port ? `:${draft.port}` : '';
  const database = draft.database.trim()
    ? `/${encodeURIComponent(draft.database.trim())}`
    : '';
  const scheme =
    draft.engine === 'clickhouse' &&
    ['require', 'verify-ca', 'verify-full'].includes(draft.tlsMode)
      ? 'clickhouse+https'
      : urlScheme(draft.engine);
  return `${scheme}://${auth}${host.trim()}${port}${database}${tlsQuery(draft)}`;
}

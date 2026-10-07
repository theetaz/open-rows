// SPDX-License-Identifier: Apache-2.0
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

/** A `password=` query parameter span. */
function querySpan(text: string): Span | undefined {
  const match = /[?&]password=([^&#]*)/.exec(text);
  if (!match) return undefined;
  const end = match.index + match[0].length;
  const start = end - (match[1] ?? '').length;
  return { start, end, cut: 'password='.length };
}

/** Remove the password and its syntax, keeping the query well formed. */
function strip(text: string, span: Span): string {
  const head = text.slice(0, span.start - span.cut);
  const tail = text.slice(span.end);
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

/** The URL for the current fields, with any password shown only as the mask. */
export function composeUrl(draft: Draft): string {
  const user = encodeURIComponent(draft.user.trim());
  const secret = hasPassword(draft) ? `:${PASSWORD_MASK}` : '';
  const auth = user || secret ? `${user}${secret}@` : '';
  const host = draft.host.includes(':') ? `[${draft.host}]` : draft.host;
  const port = draft.port ? `:${draft.port}` : '';
  const database = draft.database.trim()
    ? `/${encodeURIComponent(draft.database.trim())}`
    : '';
  const tls =
    draft.tlsMode === defaultTls(draft.host) ? '' : `?sslmode=${draft.tlsMode}`;
  return `postgres://${auth}${host.trim()}${port}${database}${tls}`;
}

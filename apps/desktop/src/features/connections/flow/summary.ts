// SPDX-License-Identifier: Apache-2.0
import type { Draft } from '../form';
import { isFileEngine } from '../engines';
import { routeOf } from '../HelperFields';
import { limitsSummary } from '../limitsForm';
import { proxySummary } from '../proxyForm';
import { sshSummary } from '../routeForm';
import { encryptLabel } from '../sqlServerTls';
import { tlsModeLabel } from '../TlsFields';
import { POLICY_LABELS } from '../../security/policies';
import { t as routes } from '../messages/routes';
import { t } from './messages';

/** A summary line: term, value, and whether the value is code-like. */
type Row = [string, string, boolean];

const ROUTE_NAMES = {
  direct: t('routeDirect'),
  cloud: t('routeCloud'),
  zero: t('routeZero'),
} as const;

/** Route, then SSH and proxy hops: `Direct · SSH ops@bastion · HTTP proxy:3128`. */
function routeText(draft: Draft): string {
  const parts = [ROUTE_NAMES[routeOf(draft.helper)]];
  if (draft.ssh.enabled) parts.push(`${t('ssh')} ${sshSummary(draft.ssh)}`);
  if (draft.proxy.enabled) parts.push(proxySummary(draft.proxy));
  return parts.join(' · ');
}

function serverRows(draft: Draft): Row[] {
  const tls =
    draft.engine === 'sqlserver'
      ? encryptLabel(draft.tlsMode)
      : tlsModeLabel(draft.tlsMode);
  return [
    [t('sumHost'), `${draft.host.trim() || '—'}:${draft.port || '—'}`, true],
    [t('sumDatabase'), draft.database.trim() || t('defaultDatabase'), true],
    [t('sumUser'), draft.user.trim() || t('currentUser'), true],
    [t('sumRoute'), routeText(draft), false],
    [t('sumTls'), tls, false],
  ];
}

function fileRows(draft: Draft): Row[] {
  return [
    [
      t('sumFile'),
      draft.fileMemory ? t('inMemory') : draft.filePath || t('notChosen'),
      false,
    ],
    [
      t('sumAccess'),
      draft.fileReadOnly ? t('readOnlyAccess') : t('readWriteAccess'),
      false,
    ],
  ];
}

/** The summary card's lines for the current draft. */
export function summaryRows(draft: Draft): Row[] {
  const limits = limitsSummary(draft.limits);
  return [
    ...(isFileEngine(draft.engine) ? fileRows(draft) : serverRows(draft)),
    [t('sumSafety'), POLICY_LABELS[draft.safety], false],
    [
      t('sumHistory'),
      draft.recordHistory ? t('recorded') : t('historyOff'),
      false,
    ],
    ...(limits === routes('defaults')
      ? []
      : [[t('advanced'), limits, false] satisfies Row]),
  ];
}

type UrlPart = {
  kind: 'scheme' | 'auth' | 'host' | 'path' | 'query';
  text: string;
};

/** A URL split for colouring: scheme, user info, host, path and query. */
export function urlParts(url: string): UrlPart[] {
  const match =
    /^([a-z0-9+-]+:(?:\/\/)?)([^@/?]*@)?([^/?]*)(\/[^?]*)?(\?.*)?$/i.exec(url);
  if (!match) return [{ kind: 'host', text: url }];
  const parts: UrlPart[] = [
    { kind: 'scheme', text: match[1] ?? '' },
    { kind: 'auth', text: match[2] ?? '' },
    { kind: 'host', text: match[3] ?? '' },
    { kind: 'path', text: match[4] ?? '' },
    { kind: 'query', text: match[5] ?? '' },
  ];
  return parts.filter((part) => part.text !== '');
}

// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button } from '@openrows/ui';
import type { Descriptor, SourceStatus } from '@openrows/ipc';
import styles from './Monitor.module.css';
import { t } from './messages/lazyDashboard';

/** Human-readable validity; absent values never acquire a numeric fallback. */
export function stateLabel(state: string): string {
  const names: Record<string, string> = {
    ok: 'OK',
    unknown: t('unknownAwaitingBaseline'),
    gap: t('gapNoObservation'),
    counterReset: t('statisticsResetAwaiting'),
    permissionDenied: t('permissionDenied'),
    unavailable: t('unavailable'),
  };
  return names[state] ?? state;
}
type Props = {
  descriptor: Descriptor | undefined;
  status: SourceStatus | undefined;
  now: number;
};
function freshness(status: SourceStatus | undefined, now: number): string {
  if (status?.lastSuccessMs == null) return t('noSuccessfulSample');
  return t('mathSSince', {
    seconds: String(
      Math.max(0, Math.floor((now - status.lastSuccessMs) / 1000)),
    ),
  });
}
function stateText(state: SourceStatus['state'] | undefined): string {
  const kind = stateLabel(state?.kind ?? 'unknown');
  return state && 'reason' in state
    ? t('stateWithReason', { state: kind, reason: state.reason })
    : kind;
}
function sourceSummary({ descriptor, status, now }: Props): string {
  const cadence = (status?.intervalMs ?? descriptor?.intervalMs ?? 0) / 1000;
  const parts = [
    stateText(status?.state ?? descriptor?.state),
    t('scopeLabel', { scope: descriptor?.scope ?? t('unknown') }),
    freshness(status, now),
    t('everySeconds', { cadence: String(cadence) }),
  ];
  if (status?.durationMs != null)
    parts.push(
      t('sampleDurationmsMs', { durationMs: status.durationMs.toFixed(1) }),
    );
  if (status?.partial) parts.push(t('partialCoverage'));
  return parts.join(' · ');
}
/** Source scope, capabilities, grants, effective cadence and freshness for each panel. */
export function SourceState(props: Props): React.JSX.Element {
  const { descriptor, status } = props;
  const state = status?.state ?? descriptor?.state;
  return (
    <div className={styles.note}>
      <p role="status">{sourceSummary(props)}</p>
      {descriptor?.coverage && <p>{descriptor.coverage}</p>}
      {descriptor?.guidance && state?.kind !== 'ok' && (
        <Guidance sql={descriptor.guidance} />
      )}
    </div>
  );
}
function Guidance({ sql }: { sql: string }): React.JSX.Element {
  const [copied, setCopied] = useState(false);
  const [error, setError] = useState<string | null>(null);
  return (
    <div>
      <pre className={styles.guidance}>{sql}</pre>
      <Button
        size="sm"
        onClick={() => {
          navigator.clipboard.writeText(sql).then(
            () => {
              setCopied(true);
            },
            (failure: unknown) => {
              setError(String(failure));
            },
          );
        }}
      >
        {copied ? t('copied') : t('copySetupGuidance')}
      </Button>
      <p>{t('reviewGuidanceDba')}</p>
      {error && <p role="alert">{error}</p>}
    </div>
  );
}

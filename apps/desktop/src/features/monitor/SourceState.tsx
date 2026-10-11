// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button } from '@openrows/ui';
import type { Descriptor, SourceStatus } from '@openrows/ipc';
import styles from './Monitor.module.css';

/** Human-readable validity; absent values never acquire a numeric fallback. */
export function stateLabel(state: string): string {
  const names: Record<string, string> = {
    ok: 'OK',
    unknown: 'Unknown — awaiting a baseline or server value',
    gap: 'Gap — no observation',
    counterReset: 'Statistics reset — awaiting a new baseline',
    permissionDenied: 'Permission denied',
    unavailable: 'Unavailable',
  };
  return names[state] ?? state;
}
type Props = {
  descriptor: Descriptor | undefined;
  status: SourceStatus | undefined;
  now: number;
};
function freshness(status: SourceStatus | undefined, now: number): string {
  if (status?.lastSuccessMs == null) return 'No successful sample';
  return `${String(Math.max(0, Math.floor((now - status.lastSuccessMs) / 1000)))} s since last successful sample`;
}
function sourceSummary({ descriptor, status, now }: Props): string {
  const state = status?.state ?? descriptor?.state;
  const reason = state && 'reason' in state ? `: ${state.reason}` : '';
  const cadence = (status?.intervalMs ?? descriptor?.intervalMs ?? 0) / 1000;
  const duration = sampleDuration(status);
  return `${stateLabel(state?.kind ?? 'unknown')}${reason} · ${descriptor?.scope ?? 'Unknown'} scope · ${freshness(status, now)} · Every ${String(cadence)} s${duration}${partialCoverage(status)}`;
}
function partialCoverage(status: SourceStatus | undefined): string {
  return status?.partial ? ' · Partial coverage' : '';
}
function sampleDuration(status: SourceStatus | undefined): string {
  return status?.durationMs == null
    ? ''
    : ` · Sample ${status.durationMs.toFixed(1)} ms`;
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
        {copied ? 'Copied' : 'Copy setup guidance'}
      </Button>
      <p>
        Review this guidance with your DBA. OpenRows never executes it
        automatically.
      </p>
      {error && <p role="alert">{error}</p>}
    </div>
  );
}

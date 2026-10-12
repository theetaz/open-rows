// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { investigateStatement } from '@openrows/ipc';
import type { Investigation, MonitorTarget } from '@openrows/ipc';
import type { MonitorTable } from './monitorData';
import { MetricChart } from './MetricChart';
import { AdvisorList } from './AdvisorList';
import { useAdvisor } from './useAdvisor';
import styles from './Monitor.module.css';

/** Per-query evidence uses real weighted deltas and live-window trends; no source percentiles exist. */
export function QueryInvestigation(props: {
  target: MonitorTarget;
  statementId: string;
  revision: string;
  now: number;
  production: boolean;
  activity: MonitorTable | undefined;
  onSession: (pid: string) => void;
  onEvidence: (link: string) => void;
}): React.JSX.Element {
  const [detail, setDetail] = useState<Investigation | null>(null);
  const advisor = useAdvisor(props.target, props.revision);
  const { profileId, database } = props.target;
  useEffect(() => {
    let alive = true;
    investigateStatement({ profileId, database }, props.statementId).then(
      (value) => {
        if (alive) setDetail(value);
      },
      () => {
        if (alive) setDetail(null);
      },
    );
    return () => {
      alive = false;
    };
  }, [profileId, database, props.statementId, props.revision]);
  if (!detail)
    return <p role="status">Query evidence unavailable; start monitoring.</p>;
  return (
    <section aria-label="Query investigation">
      <QueryStatistics detail={detail} now={props.now} />
      <IntervalMetrics detail={detail} />
      <RelatedSessions
        query={detail.query}
        table={props.activity}
        database={props.target.database}
        onSession={props.onSession}
      />
      {advisor.error && <p role="status">{advisor.error}</p>}
      <AdvisorList
        key={`${props.target.profileId}:${props.target.database}`}
        target={props.target}
        now={props.now}
        production={props.production}
        suggestions={advisor.suggestions.filter(
          (card) => card.statementId === props.statementId,
        )}
        querySql={detail.query}
        onEvidence={props.onEvidence}
      />
    </section>
  );
}
function QueryStatistics({
  detail,
  now,
}: {
  detail: Investigation;
  now: number;
}): React.JSX.Element {
  const points = detail.intervals.flatMap((interval) =>
    interval.collectedMs === null
      ? []
      : [
          {
            metric: 'mean_ms',
            collectedMs: interval.collectedMs,
            state: interval.state.kind,
            value:
              interval.state.kind === 'ok' &&
              interval.calls &&
              interval.totalMs !== null
                ? interval.totalMs / interval.calls
                : null,
          },
        ],
  );
  return (
    <>
      <h3>Query investigation · {detail.statementId}</h3>
      <pre>
        {detail.query ?? 'Fingerprint-only; normalized text unavailable'}
      </pre>
      <p>
        Live window: {String(detail.calls ?? 'unknown')} calls ·{' '}
        {String(detail.totalMs ?? 'unknown')} ms total · weighted mean{' '}
        {String(detail.meanMs ?? 'unknown')} ms · max since reset{' '}
        {String(detail.maxSinceResetMs ?? 'unknown')} ms.
      </p>
      <p className={styles.note}>
        p95/p99 unavailable: pg_stat_statements supplies summaries, not observed
        distributions. Per-query memory and CPU attribution are unavailable.
        Trends cover the retained live window; persisted history contains
        database aggregates only.{' '}
        {detail.partial
          ? 'Partial coverage: baselines, gaps or evictions are present.'
          : 'Complete retained interval coverage.'}
      </p>
      <MetricChart
        title="Query mean latency (ms)"
        points={points}
        range={{
          start: now - 600_000,
          end: now,
          cadence: Math.max(
            15_000,
            ...detail.intervals.flatMap((i) =>
              i.durationMs === null ? [] : [i.durationMs],
            ),
          ),
        }}
      />
      <p className={styles.note}>
        Regression: adjacent complete intervals each have ≥ 5 calls, mean ≥ 2×
        previous and increase ≥ 10 ms.
      </p>
      {detail.regressions.map((regression) => (
        <p key={regression.currentMs}>
          Regression at{' '}
          {regression.currentMs === null
            ? 'unknown'
            : new Date(regression.currentMs).toLocaleTimeString()}
          : {regression.previousMeanMs} → {regression.currentMeanMs} ms mean.
        </p>
      ))}
    </>
  );
}
function IntervalMetrics({
  detail,
}: {
  detail: Investigation;
}): React.JSX.Element {
  return (
    <table className={styles.table}>
      <caption>Query interval deltas</caption>
      <thead>
        <tr>
          {[
            'Collected',
            'State',
            'Calls',
            'Total ms',
            'Rows returned/affected',
            'Buffer hits',
            'Buffer reads',
            'Temp writes (blocks)',
            'Temp reads (blocks)',
          ].map((name) => (
            <th key={name} scope="col">
              {name}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {detail.intervals.map((interval) => (
          <tr key={interval.collectedMs}>
            <td>
              {interval.collectedMs === null
                ? 'unknown'
                : new Date(interval.collectedMs).toLocaleTimeString()}
            </td>
            <td>{interval.state.kind}</td>
            {[
              interval.calls,
              interval.totalMs,
              interval.rows,
              interval.sharedHit,
              interval.sharedRead,
              interval.tempWritten,
              interval.tempRead,
            ].map((value, index) => (
              <td key={index}>{value === null ? '—' : value.toFixed(2)}</td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  );
}
function RelatedSessions(props: {
  query: string | null;
  database: string;
  table: MonitorTable | undefined;
  onSession: (pid: string) => void;
}): React.JSX.Element {
  const sessions =
    props.table?.rows.filter(
      (row) =>
        props.query &&
        row.query === props.query &&
        row.database === props.database,
    ) ?? [];
  return (
    <section aria-label="Related waits and blockers">
      <h4>Related waits and blockers</h4>
      <p className={styles.note}>
        Approximate current-sample association by normalized SQL; identities and
        cross-source snapshots may differ.
      </p>
      {!sessions.length && (
        <p>No matching session in the visible activity sample.</p>
      )}
      {sessions.map((row) => (
        <p key={String(row.pid)}>
          <button
            type="button"
            onClick={() => {
              props.onSession(String(row.pid));
            }}
          >
            Session {String(row.pid)}
          </button>{' '}
          · wait {String(row.wait ?? 'none')} · query age{' '}
          {String(row.age_ms ?? 'unknown')} ms · blockers{' '}
          {String(row.blockers ?? 'none')}
        </p>
      ))}
    </section>
  );
}

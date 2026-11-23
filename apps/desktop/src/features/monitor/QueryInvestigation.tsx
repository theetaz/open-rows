// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { investigateStatement } from '@openrows/ipc';
import type { Investigation, MonitorTarget } from '@openrows/ipc';
import type { MonitorTable } from './monitorData';
import { MetricChart } from './MetricChart';
import { AdvisorList } from './AdvisorList';
import { useAdvisor } from './useAdvisor';
import styles from './Monitor.module.css';
import parts from './MonitorParts.module.css';
import { rich, t } from './messages/lazyInvestigation';

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
  if (!detail) return <p role="status">{t('queryEvidenceUnavailable')}</p>;
  return (
    <section aria-label={t('queryInvestigation')}>
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
      <h3>
        {t('queryInvestigationStatementid', {
          statementId: detail.statementId,
        })}
      </h3>
      <pre>
        {detail.query ?? 'Fingerprint-only; normalized text unavailable'}
      </pre>
      <p>
        {t('liveWindowCalls', {
          calls: String(detail.calls ?? t('unknown')),
          totalMs: String(detail.totalMs ?? t('unknown')),
          meanMs: String(detail.meanMs ?? t('unknown')),
          maxSinceResetMs: String(detail.maxSinceResetMs ?? t('unknown')),
        })}
      </p>
      <p className={styles.note}>
        {detail.partial ? t('p95P99Unavailable') : t('p95P99UnavailablePg')}
      </p>
      <MetricChart
        title={t('queryMeanLatency')}
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
      <p className={styles.note}>{t('regressionAdjacentComplete')}</p>
      {detail.regressions.map((regression) => (
        <p
          key={regression.currentMs}
          className={styles.notice}
          data-tone="warning"
        >
          {rich('regressionValuePreviousmeanms', {
            value:
              regression.currentMs === null
                ? t('unknown')
                : new Date(regression.currentMs).toLocaleTimeString(),
            previousMeanMs: regression.previousMeanMs,
            currentMeanMs: regression.currentMeanMs,
          })}
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
      <caption>{t('queryIntervalDeltas')}</caption>
      <thead>
        <tr>
          {[
            t('collected'),
            t('state'),
            t('calls'),
            t('totalMs'),
            t('rowsReturnedAffected'),
            t('bufferHits'),
            t('bufferReads'),
            t('tempWritesBlocks'),
            t('tempReadsBlocks'),
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
                ? t('unknown')
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
    <section aria-label={t('relatedWaitsBlockers')}>
      <h4>{t('relatedWaitsBlockers')}</h4>
      <p className={styles.note}>{t('approximateCurrentSample')}</p>
      {!sessions.length && <p>{t('noMatchingSession')}</p>}
      {sessions.map((row) => (
        <p key={String(row.pid)}>
          <button
            type="button"
            className={parts.linkButton}
            onClick={() => {
              props.onSession(String(row.pid));
            }}
          >
            {t('sessionPid', { pid: String(row.pid) })}
          </button>{' '}
          {t('waitWaitQuery', {
            wait: String(row.wait ?? t('none')),
            age_ms: String(row.age_ms ?? t('unknown')),
            blockers: String(row.blockers ?? t('none')),
          })}
        </p>
      ))}
    </section>
  );
}

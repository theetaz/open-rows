// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense, useState } from 'react';
import type { MonitorTarget } from '@openrows/ipc';
import { Button } from '@openrows/ui';
import type { MonitorTable } from './monitorData';
import { MonitorGrid } from './MonitorGrid';
const QueryReview = lazy(() =>
  import('./QueryReview').then((module) => ({ default: module.QueryReview })),
);
const QueryInvestigation = lazy(() =>
  import('./QueryInvestigation').then((module) => ({
    default: module.QueryInvestigation,
  })),
);
import styles from './Monitor.module.css';
import { t } from './messages/lazyInvestigation';
type Props = {
  table: MonitorTable;
  target: MonitorTarget;
  production: boolean;
  now: number;
  revision: string;
  activity: MonitorTable | undefined;
  initialStatementId?: string | undefined;
  /** Monitor Explain runs a PostgreSQL diagnostic session; other engines use the editor. */
  explain?: boolean;
  onSession: (pid: string) => void;
  onEvidence: (link: string) => void;
  onOpenSql: (sql: string) => void;
};

/** Covered live-window rankings, sortable interval metrics and explicit redacted-SQL actions. */
export function TopQueries(props: Props): React.JSX.Element {
  const [selected, setSelected] = useState<MonitorTable['rows'][number] | null>(
    null,
  );
  const row =
    selected &&
    props.table.rows.find(
      (candidate) => candidate.statement_id === selected.statement_id,
    );
  const [planRevision, setPlanRevision] = useState(0);
  const chosen =
    row ??
    props.table.rows.find((r) => r.statement_id === props.initialStatementId);
  const statementId = String(
    chosen?.statement_id ?? props.initialStatementId ?? '',
  );
  const sql = typeof chosen?.query === 'string' ? chosen.query : null;
  return (
    <>
      <p className={styles.note}>{t('callsTotalTime')}</p>
      <MonitorGrid
        label={t('topQueries')}
        table={props.table}
        onRow={setSelected}
      />
      <QueryActions
        key={statementId}
        {...props}
        sql={sql}
        statementId={statementId}
        onPlan={() => {
          setPlanRevision((value) => value + 1);
        }}
      />
      {statementId && (
        <Suspense fallback={null}>
          <QueryInvestigation
            key={statementId}
            {...props}
            statementId={statementId}
            revision={`${props.revision}:${String(planRevision)}`}
          />
        </Suspense>
      )}
    </>
  );
}
function QueryActions(
  props: Omit<Props, 'table'> & {
    sql: string | null;
    statementId: string;
    onPlan: () => void;
  },
): React.JSX.Element {
  const { sql } = props;
  const [review, setReview] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [error, setError] = useState<string | null>(null);
  return (
    <>
      <div className={styles.actions}>
        <Button
          size="sm"
          disabled={!sql}
          onClick={() => {
            if (sql)
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
          {copied ? t('copied') : t('copyNormalisedSql')}
        </Button>
        <Button
          size="sm"
          disabled={!sql}
          onClick={() => {
            if (sql)
              props.onOpenSql(
                `-- Review this normalised statement and supply redacted literals/bindings before running.\n${sql}`,
              );
          }}
        >
          {t('openEditorReview')}
        </Button>
        <ExplainAction
          sql={sql}
          enabled={props.explain !== false}
          onReview={setReview}
        />
        {!sql && props.explain !== false && (
          <span className={styles.note}>{t('selectStatementSafely')}</span>
        )}
      </div>
      {error && <p role="alert">{error}</p>}
      {review && (
        <Suspense fallback={null}>
          <QueryReview
            sql={review}
            target={props.target}
            statementId={props.statementId}
            production={props.production}
            onPlan={props.onPlan}
            onClose={() => {
              setReview(null);
            }}
          />
        </Suspense>
      )}
    </>
  );
}
function ExplainAction(props: {
  sql: string | null;
  enabled: boolean;
  onReview: (sql: string) => void;
}): React.JSX.Element {
  const { sql } = props;
  if (!props.enabled)
    return <span className={styles.note}>{t('explainMonitorPostgresql')}</span>;
  return (
    <Button
      size="sm"
      disabled={!sql}
      onClick={() => {
        if (sql) props.onReview(sql);
      }}
    >
      {t('explain')}
    </Button>
  );
}

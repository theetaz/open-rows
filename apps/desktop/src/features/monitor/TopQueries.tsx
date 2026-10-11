// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button } from '@openrows/ui';
import type { MonitorTable } from './monitorData';
import { MonitorGrid } from './MonitorGrid';
import { QueryReview } from './QueryReview';
import styles from './Monitor.module.css';
type Props = {
  table: MonitorTable;
  sessionId: string | null;
  onOpenSql: (sql: string) => void;
};

/** Latest covered statement intervals, sortable metrics and explicit redacted-SQL actions. */
export function TopQueries(props: Props): React.JSX.Element {
  const [selected, setSelected] = useState<MonitorTable['rows'][number] | null>(
    null,
  );
  const row =
    selected &&
    props.table.rows.find(
      (candidate) => candidate.statement_id === selected.statement_id,
    );
  const sql = typeof row?.query === 'string' ? row.query : null;
  return (
    <>
      <p className={styles.note}>
        Calls, total time, rows and blocks are rates for the latest valid
        interval. Window mean is total/calls. Maximum and cumulative mean are
        statistics since last reset. Rankings cover the latest interval;
        unknown/reset intervals remain unknown.
      </p>
      <MonitorGrid
        label="Top queries"
        table={props.table}
        onRow={setSelected}
      />
      <QueryActions
        key={sql}
        sql={sql}
        sessionId={props.sessionId}
        onOpenSql={props.onOpenSql}
      />
    </>
  );
}
function QueryActions(
  props: Omit<Props, 'table'> & { sql: string | null },
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
          {copied ? 'Copied' : 'Copy normalised SQL'}
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
          Open in editor for review
        </Button>
        <Button
          size="sm"
          disabled={!sql || !props.sessionId}
          onClick={() => {
            if (sql) setReview(sql);
          }}
        >
          Explain
        </Button>
        {!sql && (
          <span className={styles.note}>
            Select a statement with safely normalised SQL. Fingerprint-only rows
            cannot be explained.
          </span>
        )}
      </div>
      {error && <p role="alert">{error}</p>}
      {review && (
        <QueryReview
          sql={review}
          sessionId={props.sessionId}
          onClose={() => {
            setReview(null);
          }}
        />
      )}
    </>
  );
}

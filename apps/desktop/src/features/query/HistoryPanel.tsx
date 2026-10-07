// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { Button } from '@openrows/ui';
import { recentHistory } from '@openrows/ipc';
import type { HistoryItemDto } from '@openrows/ipc';
import styles from './Query.module.css';

const LIMIT = 100;

type Props = {
  connectionId: string | undefined;
  /** Open a history entry in a new query tab. */
  onOpen: (sql: string) => void;
};

const when = (ms: number | null): string =>
  ms === null ? '' : new Date(ms).toLocaleString();

function summary(item: HistoryItemDto): string {
  const parts = [when(item.startedAtMs)];
  if (item.durationMs !== null) parts.push(`${String(item.durationMs)} ms`);
  if (item.rows !== null) parts.push(`${String(item.rows)} rows`);
  if (item.status !== 'ok') parts.push(item.status);
  return parts.join(' · ');
}

/** Statements executed on the active connection, newest first (local store). */
export function HistoryPanel(props: Props): React.JSX.Element {
  const [items, setItems] = useState<readonly HistoryItemDto[]>([]);
  const [problem, setProblem] = useState<string | null>(null);
  const [version, setVersion] = useState(0);
  const { connectionId } = props;
  useEffect(() => {
    if (!connectionId) return;
    let live = true;
    recentHistory(connectionId, LIMIT).then(
      (found) => {
        if (!live) return;
        setItems(found);
        setProblem(null);
      },
      (error: unknown) => {
        if (live)
          setProblem(error instanceof Error ? error.message : String(error));
      },
    );
    return () => {
      live = false;
    };
  }, [connectionId, version]);
  if (!connectionId)
    return <p className={styles.empty}>No connection selected.</p>;
  return (
    <div className={styles.history}>
      <Button
        size="sm"
        variant="ghost"
        onClick={() => {
          setVersion((value) => value + 1);
        }}
      >
        Refresh history
      </Button>
      {problem && <p role="alert">{problem}</p>}
      {!items.length && !problem && <p>No statements run yet.</p>}
      <ul aria-label="Query history">
        {items.map((item) => (
          <li key={item.id ?? item.statement} data-status={item.status}>
            <button
              type="button"
              title={item.errorMessage ?? item.statement}
              onClick={() => {
                props.onOpen(item.statement);
              }}
            >
              <code>{item.statement}</code>
              <small>{summary(item)}</small>
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}

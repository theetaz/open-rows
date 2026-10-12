// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { TextField } from '@openrows/ui';
import type { ProcessDto, ProfileDto } from '@openrows/ipc';
import { ProcessGrid } from '../processes/ProcessGrid';
import { ProcessSignalSheet } from '../processes/ProcessSignalSheet';
import { useProcessSignals } from '../processes/processSignals';
import {
  DEFAULT_SORT,
  filterProcesses,
  nextSort,
  sortProcesses,
} from '../processes/processModel';
import type { MonitorTable } from './monitorData';
import styles from './Monitor.module.css';

/** Convert the shared activity window into the existing process-grid model. */
function monitoredProcesses(table: MonitorTable | undefined): ProcessDto[] {
  return (table?.rows ?? []).flatMap((row) => {
    const id = Number(row.pid);
    if (!Number.isSafeInteger(id) || id <= 0) return [];
    const text = (key: string): string | null =>
      typeof row[key] === 'string' ? row[key] : null;
    return [
      {
        id,
        state: text('state'),
        wait: text('wait'),
        durationMs: typeof row.age_ms === 'number' ? row.age_ms : null,
        query: text('query'),
        user: text('user'),
        database: text('database'),
        application: text('application'),
        queryStart: text('query_start'),
        client: null,
        current: false,
        app: text('application')?.startsWith('OpenRows') ?? false,
      },
    ];
  });
}
/** Shared Monitor activity, filter/sort and the existing confirmed cancellation policies. */
export function MonitorSessions(props: {
  table: MonitorTable | undefined;
  profile: ProfileDto;
  database: string;
  paused: boolean;
  selectedPid?: string | undefined;
}): React.JSX.Element {
  const [filter, setFilter] = useState(props.selectedPid ?? '');
  const [sort, setSort] = useState(DEFAULT_SORT);
  const [cursor, setCursor] = useState({ row: 0, col: 0 });
  const signals = useProcessSignals({
    profileId: props.profile.id,
    database: props.database,
    readOnly: props.profile.safety === 'readOnly' || props.paused,
    onSignalled: () => undefined,
  });
  const processes = sortProcesses(
    props.selectedPid && filter === props.selectedPid
      ? monitoredProcesses(props.table).filter(
          (process) => String(process.id) === props.selectedPid,
        )
      : filterProcesses(monitoredProcesses(props.table), filter),
    sort,
  );
  return (
    <>
      <TextField
        controlSize="compact"
        type="search"
        aria-label="Filter sessions"
        value={filter}
        onChange={(event) => {
          setFilter(event.target.value);
        }}
        placeholder="Filter sessions…"
      />
      <div className={styles.grid}>
        <ProcessGrid
          processes={processes.slice(0, 100)}
          cursor={{
            row: Math.min(cursor.row, Math.max(0, processes.length - 1)),
            col: cursor.col,
          }}
          onCursor={setCursor}
          sort={sort}
          onSort={(col) => {
            setSort(nextSort(sort, col));
          }}
          readOnly={props.profile.safety === 'readOnly' || props.paused}
          onSignal={signals.request}
        />
      </div>
      <p className={styles.note}>
        {String(processes.length)} sessions. Query literals are redacted.{' '}
        {processes.length > 100
          ? 'Showing the first 100; filter to narrow the list.'
          : ''}
      </p>
      {signals.error && <p role="alert">{signals.error}</p>}
      {signals.pending && (
        <ProcessSignalSheet
          pending={signals.pending}
          onCancel={signals.dismiss}
          onConfirm={signals.confirm}
        />
      )}
    </>
  );
}

// SPDX-License-Identifier: Apache-2.0
import type { ProfileDto, Source } from '@openrows/ipc';
import type { MonitorControls } from './useMonitor';
import type { MonitorTable } from './monitorData';
import { SourceState } from './SourceState';
import { MonitorGrid } from './MonitorGrid';
import { MonitorSessions } from './MonitorSessions';
import { BlockingTree } from './BlockingTree';
import { TopQueries } from './TopQueries';
import styles from './Monitor.module.css';
type Props = {
  profile: ProfileDto;
  database: string;
  sessionId: string | null;
  onOpenSql: (sql: string) => void;
  panel: Source | 'blocking';
  monitor: MonitorControls;
};
/** One live source with explicit capability/freshness and a bounded data view. */
export function SourcePanel(props: Props): React.JSX.Element {
  const source: Source = props.panel === 'blocking' ? 'activity' : props.panel;
  const data = props.monitor.data;
  const descriptor = data?.reply.capabilities.find(
    (item) => item.source === source,
  );
  const status = data?.reply.sources.find((item) => item.source === source);
  const table = data?.tables[source];
  return (
    <section aria-label={props.panel}>
      <h2>{props.panel}</h2>
      <SourceState
        descriptor={descriptor}
        status={status}
        now={props.monitor.now}
      />
      {(source === 'tables' || source === 'indexes') && (
        <p className={styles.note}>
          Statistics since last reset. Scan/tuple counters are cumulative; rate
          columns cover the latest valid interval. Vacuum/analyze times are
          unknown when the server reports NULL.
        </p>
      )}
      {status?.state.kind === 'ok' && table && (
        <SourceContent {...props} table={table} />
      )}
    </section>
  );
}
function SourceContent(
  props: Props & { table: MonitorTable },
): React.JSX.Element {
  if (props.panel === 'activity')
    return (
      <MonitorSessions
        table={props.table}
        profile={props.profile}
        database={props.database}
        paused={props.monitor.paused}
      />
    );
  if (props.panel === 'blocking') return <BlockingTree table={props.table} />;
  if (props.panel === 'statements')
    return (
      <TopQueries
        table={props.table}
        sessionId={props.sessionId}
        onOpenSql={props.onOpenSql}
      />
    );
  return (
    <>
      <MonitorGrid label={props.panel} table={props.table} />
      {props.panel === 'replication' && !props.table.rows.length && (
        <p>
          No replication senders or receivers on this server; replication lag is
          unavailable.
        </p>
      )}
    </>
  );
}

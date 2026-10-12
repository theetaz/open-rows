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
  selectedIdentity?: string | undefined;
  onSession?: ((pid: string) => void) | undefined;
  onEvidence?: (link: string) => void;
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
          {props.profile.engine === 'clickhouse'
            ? 'Active parts per table in the selected database. Many parts per partition slow merges and inserts; sizes are current, not rates.'
            : props.profile.engine === 'postgres'
              ? 'Statistics since last reset. Scan/tuple counters are cumulative; rate columns cover the latest valid interval. Vacuum/analyze times are unknown when the server reports NULL.'
              : props.profile.engine === 'sqlserver'
                ? 'Seek/scan/lookup/update counts since instance start or database open; rates cover the latest interval. Indexes unused since then are absent, which never proves an index can be dropped.'
                : 'Instrumented table/index I/O since reset; rates cover the latest interval. Unused/redundant sys evidence is optional. Zero observed reads never proves an index can be dropped. Vacuum/bloat and per-query memory are unavailable.'}
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
  if (props.panel === 'activity' && props.profile.engine !== 'clickhouse')
    return (
      <MonitorSessions
        table={props.table}
        profile={props.profile}
        database={props.database}
        paused={props.monitor.paused}
        selectedPid={props.selectedIdentity}
      />
    );
  if (props.panel === 'blocking')
    return <BlockingTree table={props.table} onSession={props.onSession} />;
  if (props.panel === 'statements')
    return (
      <TopQueries
        table={props.table}
        target={{ profileId: props.profile.id, database: props.database }}
        production={props.profile.env.kind === 'prod'}
        now={props.monitor.now}
        revision={`${props.monitor.data?.reply.generation ?? ''}:${props.monitor.data?.reply.sources.map((s) => `${s.source}:${String(s.sequence)}:${s.state.kind}`).join('|') ?? ''}:${String(Math.floor(props.monitor.now / 5000))}`}
        activity={props.monitor.data?.tables.activity}
        initialStatementId={props.selectedIdentity}
        explain={props.profile.engine === 'postgres'}
        onSession={props.onSession ?? (() => undefined)}
        onEvidence={props.onEvidence ?? (() => undefined)}
        onOpenSql={props.onOpenSql}
      />
    );
  return (
    <>
      <MonitorGrid
        label={props.panel}
        table={
          props.selectedIdentity
            ? {
                ...props.table,
                rows: props.table.rows.filter(
                  (row) =>
                    String(
                      props.panel === 'indexes'
                        ? row.index_id
                        : row.relation_id,
                    ) === props.selectedIdentity,
                ),
              }
            : props.table
        }
      />
      {props.panel === 'replication' && !props.table.rows.length && (
        <p>
          No replication senders or receivers on this server; replication lag is
          unavailable.
        </p>
      )}
    </>
  );
}

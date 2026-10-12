// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense, useEffect, useRef, useState } from 'react';
import { SegmentedControl } from '@openrows/ui';
import type { EngineDto, ProfileDto } from '@openrows/ipc';
import { useSecurity } from '../security/useSecurity';
import { useMonitor } from './useMonitor';
import { useMonitorHistory } from './useMonitorHistory';
import { MonitorHeader } from './MonitorHeader';
import { MonitorOverview } from './MonitorOverview';
import { SourcePanel } from './SourcePanel';
import { HistoryControls } from './HistoryControls';
import { IncidentsPanel } from './IncidentsPanel';
import { CLICKHOUSE_HIDDEN, CLICKHOUSE_LABELS } from './clickhouseMetrics';
import { reportConnection } from './report/connection';
import styles from './Monitor.module.css';
const AdvisorPanel = lazy(() =>
  import('./AdvisorPanel').then((module) => ({ default: module.AdvisorPanel })),
);
const BoardsPanel = lazy(() =>
  import('./boards/BoardsPanel').then((module) => ({
    default: module.BoardsPanel,
  })),
);

const PANELS = [
  { value: 'overview', label: 'Overview' },
  { value: 'incidents', label: 'Incidents' },
  { value: 'activity', label: 'Sessions' },
  { value: 'blocking', label: 'Blocking' },
  { value: 'statements', label: 'Top queries' },
  { value: 'advisor', label: 'Advisor' },
  { value: 'tables', label: 'Tables' },
  { value: 'indexes', label: 'Indexes' },
  { value: 'replication', label: 'Replication' },
  { value: 'io', label: 'I/O' },
  { value: 'boards', label: 'Boards' },
] as const;
type Panel = (typeof PANELS)[number]['value'];
/** MySQL/MariaDB table, index and file sources are instrumented I/O summaries. */
const MYSQL_LABELS: Partial<Record<Panel, string>> = {
  tables: 'Table I/O',
  indexes: 'Index I/O',
  io: 'File I/O',
};
/** SQL Server tables/indexes are usage DMVs; the I/O slot carries wait statistics. */
const SQLSERVER_LABELS: Partial<Record<Panel, string>> = {
  tables: 'Table usage',
  indexes: 'Index usage',
  io: 'Waits',
};
/** Embedded and remote libSQL see only OpenRows' own statements. */
const EMBEDDED_PANELS: ReadonlySet<Panel> = new Set([
  'overview',
  'incidents',
  'statements',
]);
const ENGINE_NAMES: Partial<Record<EngineDto, string>> = {
  postgres: 'PostgreSQL',
  mysql: 'MySQL',
  mariadb: 'MariaDB',
  sqlserver: 'SQL Server',
  clickhouse: 'ClickHouse',
  sqlite: 'SQLite',
  'libsql-file': 'libSQL file',
  libsql: 'libSQL',
  duckdb: 'DuckDB',
};
type Props = {
  profile: ProfileDto;
  database: string;
  connected: boolean;
  sessionId: string | null;
  onOpenSql: (sql: string) => void;
};

/** Lazy per-connection Monitor; lock/disconnect/unmount stop collection and hide data. */
export function MonitorTab(props: Props): React.JSX.Element {
  const security = useSecurity();
  const target = { profileId: props.profile.id, database: props.database };
  const monitor = useMonitor(target, props.connected && !security.locked);
  const history = useMonitorHistory(target, !security.locked);
  const [panel, setPanel] = useState<Panel>('overview');
  const [selectedIdentity, setSelectedIdentity] = useState<string>();
  const evidence = (link: string): void => {
    const destination = evidenceTarget(link);
    if (destination) {
      setPanel(destination.panel);
      setSelectedIdentity(destination.identity);
    }
  };
  const unlocked = useRef(false);
  const { setPaused } = monitor;
  useEffect(() => {
    if (!security.locked) unlocked.current = true;
    else if (unlocked.current) setPaused(true);
  }, [security.locked, setPaused]);
  if (security.locked)
    return <p role="status">Monitor data hidden while the app is locked.</p>;
  if (!props.connected)
    return (
      <p role="status">
        Disconnected. Connect to start monitoring; collection has stopped.
      </p>
    );
  return (
    <section
      className={styles.workspace}
      aria-label={`Monitor on ${props.database}`}
    >
      <MonitorHeader
        target={target}
        label={`${props.profile.name} · ${props.database} · ${ENGINE_NAMES[props.profile.engine] ?? props.profile.engine}`}
        connection={reportConnection(props.profile, props.database)}
        monitor={monitor}
        history={history}
      />
      <nav className={styles.navigation}>
        <SegmentedControl
          aria-label="Monitor panel"
          options={panels(props.profile.engine)}
          value={panel}
          onValueChange={(value) => {
            setPanel(value);
            setSelectedIdentity(undefined);
          }}
          size="compact"
        />
      </nav>
      <MonitorContent
        {...props}
        panel={panel}
        monitor={monitor}
        history={history}
        selectedIdentity={selectedIdentity}
        onEvidence={evidence}
        onSession={(pid) => {
          setPanel('activity');
          setSelectedIdentity(pid);
        }}
      />
    </section>
  );
}

function MonitorContent(
  props: Props & {
    panel: Panel;
    monitor: ReturnType<typeof useMonitor>;
    history: ReturnType<typeof useMonitorHistory>;
    selectedIdentity?: string | undefined;
    onEvidence: (link: string) => void;
    onSession: (pid: string) => void;
  },
): React.JSX.Element {
  const { panel, monitor, history } = props;
  return (
    <div className={styles.panel}>
      {monitor.error && (
        <p role="alert" className={styles.error}>
          {monitor.error}
        </p>
      )}
      {history.error && <p role="alert">{history.error}</p>}
      {panel === 'boards' ? (
        <Suspense fallback={null}>
          <BoardsPanel
            target={{ profileId: props.profile.id, database: props.database }}
            connection={reportConnection(props.profile, props.database)}
            active={props.connected}
          />
        </Suspense>
      ) : panel === 'advisor' ? (
        <Suspense fallback={null}>
          <AdvisorPanel
            target={{ profileId: props.profile.id, database: props.database }}
            now={monitor.now}
            production={props.profile.env.kind === 'prod'}
            revision={`${liveRevision(monitor)}:${String(Math.floor(monitor.now / 5000))}`}
            onEvidence={props.onEvidence}
          />
        </Suspense>
      ) : panel === 'incidents' ? (
        <IncidentsPanel
          target={{ profileId: props.profile.id, database: props.database }}
          revision={liveRevision(monitor)}
        />
      ) : panel === 'overview' ? (
        <MonitorOverview
          engine={props.profile.engine}
          data={monitor.data}
          points={history.hours ? history.points : (monitor.data?.points ?? [])}
          previous={history.previous}
          now={monitor.now}
          duration={history.hours ? history.hours * 3_600_000 : 600_000}
          end={history.hours ? history.end : monitor.now}
          history={history.hours > 0}
        />
      ) : (
        <SourcePanel
          key={`${panel}:${props.selectedIdentity ?? ''}`}
          {...props}
          panel={panel}
        />
      )}
      <details>
        <summary>Local history settings</summary>
        <HistoryControls
          profileId={props.profile.id}
          onPurged={history.refresh}
        />
      </details>
    </div>
  );
}

/** Changes whenever the collector publishes a new source status. */
function liveRevision(monitor: ReturnType<typeof useMonitor>): string {
  const reply = monitor.data?.reply;
  if (!reply) return '';
  const sources = reply.sources
    .map((s) => `${s.source}:${String(s.sequence)}:${s.state.kind}`)
    .join('|');
  return `${reply.generation}:${sources}`;
}

function panels(engine: EngineDto): readonly { value: Panel; label: string }[] {
  if (engine === 'clickhouse')
    return PANELS.filter((panel) => !CLICKHOUSE_HIDDEN.has(panel.value)).map(
      (panel) => ({
        value: panel.value,
        label: CLICKHOUSE_LABELS[panel.value] ?? panel.label,
      }),
    );
  if (ownSessionsOnly(engine))
    return PANELS.filter((panel) => EMBEDDED_PANELS.has(panel.value)).map(
      (panel) =>
        panel.value === 'statements'
          ? { value: panel.value, label: 'Own queries' }
          : panel,
    );
  const labels =
    engine === 'mysql' || engine === 'mariadb'
      ? MYSQL_LABELS
      : engine === 'sqlserver'
        ? SQLSERVER_LABELS
        : null;
  if (!labels) return PANELS;
  return PANELS.map((panel) => ({
    value: panel.value,
    label: labels[panel.value] ?? panel.label,
  }));
}

/** Engines whose collector reads only OpenRows' own statements. */
function ownSessionsOnly(engine: EngineDto | undefined): boolean {
  return (
    engine === 'sqlite' ||
    engine === 'libsql-file' ||
    engine === 'libsql' ||
    engine === 'duckdb'
  );
}

function evidenceTarget(
  link: string,
): { panel: Panel; identity: string } | null {
  const [source, ...identity] = link.split(':');
  if (source === 'sessions')
    return { panel: 'activity', identity: identity.join(':') };
  if (source === 'tables' || source === 'indexes' || source === 'statements')
    return { panel: source, identity: identity.join(':') };
  return null;
}

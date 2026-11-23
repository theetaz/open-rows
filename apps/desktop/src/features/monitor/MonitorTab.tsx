// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense, useEffect, useRef, useState } from 'react';
import type { ProfileDto } from '@openrows/ipc';
import { isDocumentEngine } from '../connections/engines';
import { useSecurity } from '../security/useSecurity';
import { useMonitor } from './useMonitor';
import { useMonitorHistory } from './useMonitorHistory';
import { useIncidents } from './useIncidents';
import { MonitorHeader } from './MonitorHeader';
import { MonitorNav } from './MonitorNav';
import { MonitorOverview } from './MonitorOverview';
import { SourcePanel } from './SourcePanel';
import { HistoryControls } from './HistoryControls';
import { IncidentsView, OpenIncidentBanner } from './IncidentsPanel';
import { ENGINE_NAMES, evidenceTarget, panels } from './monitorPanels';
import type { Panel } from './monitorPanels';
import { reportConnection } from './report/connection';
import styles from './Monitor.module.css';
import layout from './MonitorLayout.module.css';
import { t } from './messages/lazyDashboard';
const InvestigationPanel = lazy(() =>
  import('./InvestigationPanel').then((module) => ({
    default: module.InvestigationPanel,
  })),
);
const AdvisorPanel = lazy(() =>
  import('./AdvisorPanel').then((module) => ({ default: module.AdvisorPanel })),
);
const BoardsPanel = lazy(() =>
  import('./boards/BoardsPanel').then((module) => ({
    default: module.BoardsPanel,
  })),
);

type Props = {
  profile: ProfileDto;
  database: string;
  connected: boolean;
  sessionId: string | null;
  onOpenSql: (sql: string) => void;
};
type Monitor = ReturnType<typeof useMonitor>;
type History = ReturnType<typeof useMonitorHistory>;
type Incidents = ReturnType<typeof useIncidents>;

/** Lazy per-connection Monitor; document engines have no collector. */
export function MonitorTab(props: Props): React.JSX.Element {
  if (isDocumentEngine(props.profile.engine))
    return (
      <p className={styles.unavailable} role="status">
        {t('monitoringAiAssistant')}
      </p>
    );
  return <EngineMonitor {...props} />;
}

/** Lock/disconnect/unmount stop collection and hide data. */
function EngineMonitor(props: Props): React.JSX.Element {
  const security = useSecurity();
  const target = { profileId: props.profile.id, database: props.database };
  const monitor = useMonitor(target, props.connected && !security.locked);
  const history = useMonitorHistory(target, !security.locked);
  const incidents = useIncidents(target, liveRevision(monitor));
  const [panel, setPanel] = useState<Panel>('overview');
  const [selectedIdentity, setSelectedIdentity] = useState<string>();
  const show = (next: Panel, identity?: string): void => {
    setPanel(next);
    setSelectedIdentity(identity);
  };
  const unlocked = useRef(false);
  const { setPaused } = monitor;
  useEffect(() => {
    if (!security.locked) unlocked.current = true;
    else if (unlocked.current) setPaused(true);
  }, [security.locked, setPaused]);
  if (security.locked) return <p role="status">{t('monitorDataHidden')}</p>;
  if (!props.connected)
    return <p role="status">{t('disconnectedConnectStart')}</p>;
  return (
    <section
      className={layout.workspace}
      aria-label={t('monitorDatabase', { database: props.database })}
    >
      <MonitorNav
        options={panels(props.profile.engine)}
        value={panel}
        openIncidents={openCount(incidents)}
        onValueChange={(value) => {
          show(value);
        }}
      />
      <div className={layout.main}>
        <MonitorHeader
          target={target}
          label={`${props.profile.name} · ${props.database} · ${ENGINE_NAMES[props.profile.engine] ?? props.profile.engine}`}
          connection={reportConnection(props.profile, props.database)}
          monitor={monitor}
          history={history}
        />
        <MonitorContent
          {...props}
          panel={panel}
          monitor={monitor}
          history={history}
          incidents={incidents}
          selectedIdentity={selectedIdentity}
          onShow={show}
        />
        <footer className={layout.statusline}>
          <span>{t('databaseStatementsI')}</span>
        </footer>
      </div>
    </section>
  );
}

type ContentProps = Props & {
  panel: Panel;
  monitor: Monitor;
  history: History;
  incidents: Incidents;
  selectedIdentity?: string | undefined;
  onShow: (panel: Panel, identity?: string) => void;
};

function MonitorContent(props: ContentProps): React.JSX.Element {
  const { monitor, history } = props;
  return (
    <div className={[layout.page, styles.content].join(' ')}>
      {monitor.error && (
        <p role="alert" className={styles.error}>
          {monitor.error}
        </p>
      )}
      {history.error && <p role="alert">{history.error}</p>}
      <PanelBody {...props} />
      <details className={styles.disclosure}>
        <summary>{t('localHistorySettings')}</summary>
        <HistoryControls
          profileId={props.profile.id}
          onPurged={history.refresh}
        />
      </details>
    </div>
  );
}

/** Follow an advisor or query evidence link to its section and row. */
function followEvidence(props: ContentProps): (link: string) => void {
  return (link) => {
    const destination = evidenceTarget(link);
    if (destination) props.onShow(destination.panel, destination.identity);
  };
}

function PanelBody(props: ContentProps): React.JSX.Element {
  const { panel } = props;
  if (panel === 'boards' || panel === 'assistant' || panel === 'advisor')
    return (
      <Suspense fallback={null}>
        <LazyPanel {...props} />
      </Suspense>
    );
  if (panel === 'incidents') return <IncidentsView {...props.incidents} />;
  if (panel === 'overview') return <Overview {...props} />;
  return (
    <SourcePanel
      key={`${panel}:${props.selectedIdentity ?? ''}`}
      {...props}
      panel={panel}
      title={panels(props.profile.engine).find((o) => o.value === panel)?.label}
      onEvidence={followEvidence(props)}
      onSession={(pid) => {
        props.onShow('activity', pid);
      }}
    />
  );
}

/** Boards, the assistant and the advisor load on first use. */
function LazyPanel(props: ContentProps): React.JSX.Element {
  const { monitor } = props;
  const target = { profileId: props.profile.id, database: props.database };
  if (props.panel === 'boards')
    return (
      <BoardsPanel
        target={target}
        connection={reportConnection(props.profile, props.database)}
        active={props.connected}
      />
    );
  if (props.panel === 'assistant')
    return (
      <InvestigationPanel
        profile={props.profile}
        database={props.database}
        connected={props.connected}
        onOpen={props.onOpenSql}
      />
    );
  return (
    <AdvisorPanel
      target={target}
      now={monitor.now}
      production={props.profile.env.kind === 'prod'}
      revision={`${liveRevision(monitor)}:${String(Math.floor(monitor.now / 5000))}`}
      onEvidence={followEvidence(props)}
    />
  );
}

/** The open-incident banner over the metric tiles. */
function Overview(props: ContentProps): React.JSX.Element {
  const { monitor, history } = props;
  return (
    <>
      <OpenIncidentBanner
        incidents={props.incidents.reply?.incidents ?? []}
        onOpen={() => {
          props.onShow('incidents');
        }}
      />
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
    </>
  );
}

function openCount(incidents: Incidents): number {
  return (incidents.reply?.incidents ?? []).filter(
    (incident) => incident.endedMs === null,
  ).length;
}

/** Changes whenever the collector publishes a new source status. */
function liveRevision(monitor: Monitor): string {
  const reply = monitor.data?.reply;
  if (!reply) return '';
  const sources = reply.sources
    .map((s) => `${s.source}:${String(s.sequence)}:${s.state.kind}`)
    .join('|');
  return `${reply.generation}:${sources}`;
}

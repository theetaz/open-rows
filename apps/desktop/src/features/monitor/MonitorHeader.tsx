// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense, useState } from 'react';
import { Icon, IconButton, SelectField } from '@openrows/ui';
import type { MonitorTarget } from '@openrows/ipc';
import type { Connection } from './report/connection';
import { BackgroundToggle } from './BackgroundToggle';
import type { MonitorControls } from './useMonitor';
import type { useMonitorHistory } from './useMonitorHistory';
import layout from './MonitorLayout.module.css';
import { t } from './messages/lazyDashboard';
const MonitorReportDialog = lazy(() =>
  import('./report/ReportEntry').then((module) => ({
    default: module.MonitorReportDialog,
  })),
);
const INTERVALS = [
  { value: '0', label: t('off') },
  ...[2000, 5000, 10000, 30000, 60000].map((value) => ({
    value: String(value),
    label: t('valueS', { value: String(value / 1000) }),
  })),
];

/**
 * The Monitor's view bar (the kit's `.viewbar`): live state and scope on the
 * left; cadence, window, background, pause and export on the right. Heavier
 * source floors remain core-owned.
 */
export function MonitorHeader(props: {
  target: MonitorTarget;
  label: string;
  connection: Connection;
  monitor: MonitorControls;
  history: ReturnType<typeof useMonitorHistory>;
}): React.JSX.Element {
  const { monitor, history } = props;
  return (
    <header className={layout.bar}>
      <span
        className={layout.liveDot}
        data-paused={monitor.paused}
        aria-hidden="true"
      />
      <strong className={layout.freshness} role="status">
        {freshness(monitor)}
      </strong>
      <span className={layout.scope}>{props.label}</span>
      <span className={layout.spacer} />
      <span className={layout.barLabel}>{t('liveInterval')}</span>
      <SelectField
        aria-label={t('monitorInterval')}
        controlSize="compact"
        value={monitor.paused ? '0' : String(monitor.interval)}
        options={INTERVALS}
        onValueChange={(value) => {
          monitor.setPaused(!Number(value));
          if (Number(value)) monitor.setInterval(Number(value));
        }}
      />
      <WindowPicker history={history} />
      <BackgroundToggle
        target={props.target}
        enabled={monitor.data?.reply.background ?? false}
        disabled={monitor.paused || !monitor.data}
      />
      <IconButton
        aria-label={monitor.paused ? t('resumeMonitor') : t('pauseMonitor')}
        title={monitor.paused ? t('resumeMonitor') : t('pauseMonitor')}
        size="sm"
        variant="ghost"
        onClick={() => {
          monitor.setPaused(!monitor.paused);
        }}
      >
        <Icon name={monitor.paused ? 'play' : 'pause'} />
      </IconButton>
      <ReportButton {...props} />
    </header>
  );
}

/** `Time window`: the live ten minutes, or stored history when it is on. */
function WindowPicker(props: {
  history: ReturnType<typeof useMonitorHistory>;
}): React.JSX.Element {
  const { history } = props;
  const options = [
    { value: '0', label: t('live10Minutes') },
    ...(history.enabled ? [1, 6, 24] : []).map((value) => ({
      value: String(value),
      label: t('historyValueH', { value: String(value) }),
    })),
  ];
  return (
    <>
      <span className={layout.barLabel}>{t('timeWindow')}</span>
      <SelectField
        aria-label={t('monitorTimeWindow')}
        controlSize="compact"
        value={String(history.hours)}
        options={options}
        onValueChange={(value) => {
          history.setHours(Number(value));
        }}
      />
      {history.hours > 0 && (
        <IconButton
          aria-label={t('refreshHistoryRange')}
          title={t('refreshHistoryRange')}
          size="sm"
          variant="ghost"
          onClick={history.refresh}
        >
          <Icon name="refresh" />
        </IconButton>
      )}
    </>
  );
}

/** "Export report…" and its dialog, loaded on first use. */
function ReportButton(
  props: Parameters<typeof MonitorHeader>[0],
): React.JSX.Element {
  const [reporting, setReporting] = useState(false);
  return (
    <>
      <IconButton
        aria-label={t('exportReport')}
        title={t('exportReport')}
        size="sm"
        variant="ghost"
        onClick={() => {
          setReporting(true);
        }}
      >
        <Icon name="download" />
      </IconButton>
      {reporting && (
        <Suspense fallback={null}>
          <MonitorReportDialog
            target={props.target}
            connection={props.connection}
            data={props.monitor.data}
            history={props.history}
            onClose={() => {
              setReporting(false);
            }}
          />
        </Suspense>
      )}
    </>
  );
}

/** Live/paused state and age of the newest successful sample. */
function freshness(monitor: MonitorControls): string {
  const times =
    monitor.data?.reply.sources.flatMap((source) =>
      source.lastSuccessMs === null ? [] : [source.lastSuccessMs],
    ) ?? [];
  const state = monitor.paused ? t('pausedRetainedSamples') : t('live');
  if (!times.length) return t('stateAwaitingFirst', { state });
  const age = Math.max(
    0,
    Math.floor((monitor.now - Math.max(...times)) / 1000),
  );
  return t('stateAgeS', { state, age: String(age) });
}

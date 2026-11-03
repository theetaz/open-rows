// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense, useState } from 'react';
import { Button } from '@openrows/ui';
import type { MonitorTarget } from '@openrows/ipc';
import type { Connection } from './report/connection';
import { BackgroundToggle } from './BackgroundToggle';
import type { MonitorControls } from './useMonitor';
import type { useMonitorHistory } from './useMonitorHistory';
import styles from './Monitor.module.css';
import { t } from './messages/lazyDashboard';
const MonitorReportDialog = lazy(() =>
  import('./report/ReportEntry').then((module) => ({
    default: module.MonitorReportDialog,
  })),
);
/** Scope and explicit cadence/window controls; heavier source floors remain core-owned. */
export function MonitorHeader(props: {
  target: MonitorTarget;
  label: string;
  connection: Connection;
  monitor: MonitorControls;
  history: ReturnType<typeof useMonitorHistory>;
}): React.JSX.Element {
  const { monitor, history } = props;
  return (
    <header className={styles.header}>
      <strong>{props.label}</strong>
      <span role="status">{freshness(monitor)}</span>
      <label>
        {t('liveInterval')}
        <select
          aria-label={t('monitorInterval')}
          value={monitor.paused ? '0' : String(monitor.interval)}
          onChange={(event) => {
            const value = Number(event.target.value);
            monitor.setPaused(!value);
            if (value) monitor.setInterval(value);
          }}
        >
          <option value="0">{t('off')}</option>
          {[2000, 5000, 10000, 30000, 60000].map((value) => (
            <option key={value} value={value}>
              {t('valueS', { value: String(value / 1000) })}
            </option>
          ))}
        </select>
      </label>
      <label>
        {t('timeWindow')}
        <select
          aria-label={t('monitorTimeWindow')}
          value={history.hours}
          onChange={(event) => {
            history.setHours(Number(event.target.value));
          }}
        >
          <option value="0">{t('live10Minutes')}</option>
          {history.enabled &&
            [1, 6, 24].map((value) => (
              <option key={value} value={value}>
                {t('historyValueH', { value: String(value) })}
              </option>
            ))}
        </select>
      </label>
      <Button
        size="sm"
        onClick={() => {
          monitor.setPaused(!monitor.paused);
        }}
      >
        {monitor.paused ? t('resumeMonitor') : t('pauseMonitor')}
      </Button>
      <BackgroundToggle
        target={props.target}
        enabled={monitor.data?.reply.background ?? false}
        disabled={monitor.paused || !monitor.data}
      />
      {history.hours > 0 && (
        <Button size="sm" onClick={history.refresh}>
          {t('refreshHistoryRange')}
        </Button>
      )}
      <ReportButton {...props} />
      <span className={styles.note}>{t('databaseStatementsI')}</span>
    </header>
  );
}

/** "Export report…" and its dialog, loaded on first use. */
function ReportButton(
  props: Parameters<typeof MonitorHeader>[0],
): React.JSX.Element {
  const [reporting, setReporting] = useState(false);
  return (
    <>
      <Button
        size="sm"
        onClick={() => {
          setReporting(true);
        }}
      >
        {t('exportReport')}
      </Button>
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

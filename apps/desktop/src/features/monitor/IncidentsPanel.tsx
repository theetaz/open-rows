// SPDX-License-Identifier: Apache-2.0
import type { MonitorTarget } from '@openrows/ipc';
import { useIncidents } from './useIncidents';
import { clock, evidence, hold, thresholds } from './incidentFormat';
import styles from './Monitor.module.css';

/** Compact open/resolved incident list with evidence; deterministic rules only. */
export function IncidentsPanel(props: {
  target: MonitorTarget;
  revision: string;
}): React.JSX.Element {
  const { reply, error } = useIncidents(props.target, props.revision);
  const incidents = reply?.incidents ?? [];
  return (
    <section aria-label="Incidents">
      <p className={styles.note}>
        Rules evaluate live samples while this monitor runs. Unknown or missing
        samples never open or resolve an incident. Notifications stay in
        OpenRows.
      </p>
      {error && <p role="alert">{error}</p>}
      {reply && !incidents.length && <p>No incidents in this app session.</p>}
      {incidents.length > 0 && (
        <table className={styles.table} aria-label="Incident list">
          <thead>
            <tr>
              <th>Status</th>
              <th>Rule</th>
              <th>Scope</th>
              <th>Started</th>
              <th>Ended</th>
              <th>Evidence</th>
            </tr>
          </thead>
          <tbody>
            {incidents.map((incident) => (
              <tr key={incident.id}>
                <td>
                  {incident.endedMs === null ? (
                    <strong>Open</strong>
                  ) : (
                    'Resolved'
                  )}
                </td>
                <td>
                  {incident.rule.title}
                  {incident.rule.severity === 'critical' && ' · critical'}
                </td>
                <td>{incident.scope === 'server' ? 'Server' : 'Database'}</td>
                <td>{clock(incident.startedMs)}</td>
                <td>
                  {incident.endedMs === null ? '—' : clock(incident.endedMs)}
                </td>
                <td>
                  {evidence(incident)}
                  <span className={styles.note}>
                    {' '}
                    · {thresholds(incident.rule)}
                  </span>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      {reply && (
        <details>
          <summary>Rules ({String(reply.rules.length)})</summary>
          <table className={styles.table}>
            <tbody>
              {reply.rules.map((rule) => (
                <tr key={rule.id}>
                  <td>{rule.title}</td>
                  <td>{thresholds(rule)}</td>
                  <td>
                    Hold {hold(rule.fireAfter)}; recover {hold(rule.clearAfter)}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </details>
      )}
    </section>
  );
}

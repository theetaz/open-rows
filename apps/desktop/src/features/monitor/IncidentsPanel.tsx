// SPDX-License-Identifier: Apache-2.0
import type { MonitorTarget } from '@openrows/ipc';
import { useIncidents } from './useIncidents';
import { clock, evidence, hold, thresholds } from './incidentFormat';
import styles from './Monitor.module.css';
import { t } from './messages/lazyDashboard';

function EvidenceCell({
  incident,
}: {
  incident: Parameters<typeof evidence>[0];
}): React.JSX.Element {
  return (
    <td>
      {evidence(incident)}
      <span className={styles.note}> · {thresholds(incident.rule)}</span>
    </td>
  );
}

/** Compact open/resolved incident list with evidence; deterministic rules only. */
export function IncidentsPanel(props: {
  target: MonitorTarget;
  revision: string;
}): React.JSX.Element {
  const { reply, error } = useIncidents(props.target, props.revision);
  const incidents = reply?.incidents ?? [];
  return (
    <section aria-label={t('incidents')}>
      <p className={styles.note}>{t('rulesEvaluateLive')}</p>
      {error && <p role="alert">{error}</p>}
      {reply && !incidents.length && <p>{t('noIncidentsApp')}</p>}
      {incidents.length > 0 && (
        <table className={styles.table} aria-label={t('incidentList')}>
          <thead>
            <tr>
              <th>{t('status')}</th>
              <th>{t('rule')}</th>
              <th>{t('scope')}</th>
              <th>{t('started')}</th>
              <th>{t('ended')}</th>
              <th>{t('evidence')}</th>
            </tr>
          </thead>
          <tbody>
            {incidents.map((incident) => (
              <tr key={incident.id}>
                <td>
                  {incident.endedMs === null ? (
                    <strong>{t('open')}</strong>
                  ) : (
                    t('resolved')
                  )}
                </td>
                <td>
                  {incident.rule.severity === 'critical'
                    ? t('titleCritical', { title: incident.rule.title })
                    : incident.rule.title}
                </td>
                <td>
                  {incident.scope === 'server' ? t('server') : t('database')}
                </td>
                <td>{clock(incident.startedMs)}</td>
                <td>
                  {incident.endedMs === null ? '—' : clock(incident.endedMs)}
                </td>
                <EvidenceCell incident={incident} />
              </tr>
            ))}
          </tbody>
        </table>
      )}
      {reply && (
        <details>
          <summary>
            {t('rulesCount', { count: String(reply.rules.length) })}
          </summary>
          <table className={styles.table}>
            <tbody>
              {reply.rules.map((rule) => (
                <tr key={rule.id}>
                  <td>{rule.title}</td>
                  <td>{thresholds(rule)}</td>
                  <td>
                    {t('holdFireafterRecover', {
                      fireAfter: hold(rule.fireAfter),
                      clearAfter: hold(rule.clearAfter),
                    })}
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

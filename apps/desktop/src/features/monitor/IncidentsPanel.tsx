// SPDX-License-Identifier: Apache-2.0
import { Badge, Button, Icon } from '@openrows/ui';
import type { Incident, IncidentsReply, MonitorTarget } from '@openrows/ipc';
import { useIncidents } from './useIncidents';
import { clock, evidence, hold, thresholds } from './incidentFormat';
import styles from './Monitor.module.css';
import { t } from './messages/lazyDashboard';

function EvidenceCell({ incident }: { incident: Incident }): React.JSX.Element {
  return (
    <td>
      {evidence(incident)}
      <span className={styles.note}> · {thresholds(incident.rule)}</span>
    </td>
  );
}

function ruleTitle(incident: Incident): string {
  return incident.rule.severity === 'critical'
    ? t('titleCritical', { title: incident.rule.title })
    : incident.rule.title;
}

function IncidentRow({ incident }: { incident: Incident }): React.JSX.Element {
  const open = incident.endedMs === null;
  return (
    <tr>
      <td>
        <Badge tone={open ? 'danger' : 'neutral'}>
          {open ? t('open') : t('resolved')}
        </Badge>
      </td>
      <td>
        {open ? <strong>{ruleTitle(incident)}</strong> : ruleTitle(incident)}
      </td>
      <td>{incident.scope === 'server' ? t('server') : t('database')}</td>
      <td className={styles.number}>{clock(incident.startedMs)}</td>
      <td className={styles.number}>{open ? '—' : clock(incident.endedMs)}</td>
      <EvidenceCell incident={incident} />
    </tr>
  );
}

function RuleList({ reply }: { reply: IncidentsReply }): React.JSX.Element {
  return (
    <details className={styles.disclosure}>
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
  );
}

/** Compact open/resolved incident list with evidence; deterministic rules only. */
export function IncidentsView(props: {
  reply: IncidentsReply | null;
  error: string | null;
}): React.JSX.Element {
  const { reply } = props;
  const incidents = reply?.incidents ?? [];
  return (
    <section aria-label={t('incidents')}>
      <p className={styles.lead}>{t('rulesEvaluateLive')}</p>
      {props.error && <p role="alert">{props.error}</p>}
      {reply && !incidents.length && (
        <p className={styles.empty}>{t('noIncidentsApp')}</p>
      )}
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
              <IncidentRow key={incident.id} incident={incident} />
            ))}
          </tbody>
        </table>
      )}
      {reply && <RuleList reply={reply} />}
    </section>
  );
}

/** The incident list reading its own incidents (outside the Monitor tab). */
export function IncidentsPanel(props: {
  target: MonitorTarget;
  revision: string;
}): React.JSX.Element {
  const { reply, error } = useIncidents(props.target, props.revision);
  return <IncidentsView reply={reply} error={error} />;
}

/** The newest open incident above the Overview, with a way to its list. */
export function OpenIncidentBanner(props: {
  incidents: readonly Incident[];
  onOpen: () => void;
}): React.JSX.Element | null {
  const open = props.incidents.filter((incident) => incident.endedMs === null);
  const [first] = open;
  if (!first) return null;
  return (
    <div className={styles.notice} data-tone="danger" role="status">
      <Icon name="warning" />
      <div className={styles.noticeBody}>
        <strong>{ruleTitle(first)}</strong>
        {evidence(first)} · {clock(first.startedMs)}
        {open.length > 1 &&
          ` · ${t('openIncidentsMore', { count: String(open.length - 1) })}`}
      </div>
      <Button size="sm" onClick={props.onOpen}>
        {t('showIncident')}
      </Button>
    </div>
  );
}

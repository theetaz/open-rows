// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { Button } from '@openrows/ui';
import { onMonitorIncident } from '@openrows/ipc';
import type { MonitorIncident } from '@openrows/ipc';
import { useSecurity } from '../security/useSecurity';
import { formatValue } from './incidentFormat';
import styles from './IncidentNotice.module.css';
import { t } from './messages/notices';

type Notice = MonitorIncident & { key: number };

/** In-app incident notifications only; hidden and cleared while locked. */
export function IncidentNotice(): React.JSX.Element | null {
  const { locked } = useSecurity();
  const [notices, setNotices] = useState<Notice[]>([]);
  useEffect(() => {
    if (locked) return undefined;
    let live = true;
    let unlisten: (() => void) | undefined;
    let sequence = 0;
    void onMonitorIncident((event) => {
      sequence += 1;
      const notice = { ...event, key: sequence };
      setNotices((previous) => [notice, ...previous].slice(0, 3));
    }).then((stop) => {
      if (live) unlisten = stop;
      else stop();
    });
    return () => {
      live = false;
      unlisten?.();
      // Locking unmounts the subscription; nothing shown before the lock survives it.
      setNotices([]);
    };
  }, [locked]);
  if (locked || !notices.length) return null;
  return (
    <ul
      className={styles.notices}
      role="status"
      aria-label={t('monitorIncidents')}
    >
      {notices.map((notice) => (
        <li key={notice.key} className={styles.notice}>
          <span>{describe(notice)}</span>
          <Button
            size="sm"
            onClick={() => {
              setNotices((previous) =>
                previous.filter((item) => item.key !== notice.key),
              );
            }}
          >
            {t('dismiss')}
          </Button>
        </li>
      ))}
    </ul>
  );
}

function describe(notice: Notice): string {
  const { kind, incident } = notice.transition;
  const value = formatValue(incident.rule.metric, incident.last.value);
  const state = kind === 'opened' ? t('incidentOpened') : t('incidentResolved');
  return t('stateTitleDatabase', {
    state,
    title: incident.rule.title,
    database: notice.target.database,
    value,
  });
}

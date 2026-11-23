// SPDX-License-Identifier: Apache-2.0
import { memo } from 'react';
import { Badge } from '@openrows/ui';
import type { ActivityEvent } from '@openrows/ipc';
import { useProfiles } from '../connections/useProfiles';
import { AppGlyph } from './AppGlyph';
import { clientName, isAgentApp, outcomeLabel, relativeTime } from './appNames';
import styles from './Apps.module.css';
import { t } from './messages';

const TONES = { ok: 'success', refused: 'warning', failed: 'danger' } as const;

/** Keys survive new events arriving on top; repeats of one call are numbered. */
function stableKey(event: ActivityEvent, counts: Map<string, number>): string {
  const base = `${String(event.at)}|${event.token}|${event.tool}|${event.connection ?? ''}`;
  const seen = counts.get(base) ?? 0;
  counts.set(base, seen + 1);
  return `${base}|${String(seen)}`;
}

const Row = memo(function Row(props: {
  event: ActivityEvent;
  connection: string | null;
  now: number;
}): React.JSX.Element {
  const { event } = props;
  return (
    <li className={styles.activityRow}>
      {isAgentApp(event.app) ? (
        <AppGlyph app={event.app} size="small" />
      ) : (
        <span className={styles.glyphGap} aria-hidden="true" />
      )}
      <span className={styles.activityMain}>
        <code className={styles.tool}>{event.tool}</code>
        <span className={styles.activitySub}>
          {[clientName(event.app), props.connection]
            .filter(Boolean)
            .join(' · ')}
        </span>
      </span>
      <time
        className={styles.activityTime}
        dateTime={new Date(event.at * 1000).toISOString()}
      >
        {relativeTime(event.at, props.now)}
      </time>
      <Badge tone={TONES[event.outcome]}>{outcomeLabel(event.outcome)}</Badge>
    </li>
  );
});

/** Time, app, tool, connection and outcome of recent external tool calls. */
export function ActivityList(props: {
  events: readonly ActivityEvent[];
  now: number;
  limit?: number;
}): React.JSX.Element {
  const { profiles } = useProfiles();
  const names = new Map(profiles.map((profile) => [profile.id, profile.name]));
  const shown = props.events.slice(0, props.limit ?? 20);
  if (!shown.length) return <p className={styles.empty}>{t('noActivity')}</p>;
  const counts = new Map<string, number>();
  return (
    <ol className={styles.activity} aria-label={t('activityList')}>
      {shown.map((event) => (
        <Row
          key={stableKey(event, counts)}
          event={event}
          connection={
            event.connection
              ? (names.get(event.connection) ?? event.connection)
              : null
          }
          now={props.now}
        />
      ))}
    </ol>
  );
}

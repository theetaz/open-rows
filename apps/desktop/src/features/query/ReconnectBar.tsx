// SPDX-License-Identifier: Apache-2.0
import { Button } from '@openrows/ui';
import type { ProfileDto } from '@openrows/ipc';
import { needsConsent } from '../connections';
import type { Connections, OpenedDatabase } from '../connections';
import styles from './Query.module.css';
import { t } from './initialMessages';

type Status = ReturnType<Connections['statusOf']>;

/** What the bar says for a connection that is not up; `null` while it is. */
function message(profile: ProfileDto, status: Status): string | null {
  if (status.state === 'failed')
    return t('couldNotConnect', {
      name: profile.name,
      message: status.message,
    });
  if (status.state !== 'disconnected') return null;
  if (needsConsent(profile))
    return t('nameNotConnected', { name: profile.name });
  return t('nameNotConnected2', { name: profile.name });
}

/** What the bar says about the tab's database session; `null` while it is up. */
function databaseMessage(database: OpenedDatabase | undefined): string | null {
  if (database?.state === 'reconnecting')
    return t('connectionNameWas', { name: database.name });
  if (database?.state === 'failed' && database.lost)
    return t('lostConnectionName', {
      name: database.name,
      message: database.message,
    });
  return null;
}

/**
 * Restored (or dropped) tabs of a connection that is not up: why, and a
 * Reconnect button that runs the normal connect flow. While the connection
 * is up, the tab database's lost or reconnecting session (M1-08).
 */
export function ReconnectBar(props: {
  profile: ProfileDto;
  status: Status;
  onConnect: () => void;
  /** The tab's opened database, when the connection is up. */
  database?: OpenedDatabase | undefined;
  /** Open the tab's database again after its connection was lost. */
  onReopen?: (() => void) | undefined;
}): React.JSX.Element | null {
  const text = message(props.profile, props.status);
  if (text)
    return (
      <div className={styles.reconnect} role="status">
        <span title={text}>{text}</span>
        <Button size="sm" onClick={props.onConnect}>
          {props.status.state === 'failed' ? t('reconnect') : t('connect')}
        </Button>
      </div>
    );
  const lost = databaseMessage(props.database);
  if (!lost) return null;
  return (
    <div className={styles.reconnect} role="status">
      <span title={lost}>{lost}</span>
      {props.database?.state === 'failed' && props.onReopen && (
        <Button size="sm" onClick={props.onReopen}>
          {t('reconnect')}
        </Button>
      )}
    </div>
  );
}

// SPDX-License-Identifier: Apache-2.0
import { Button } from '@openrows/ui';
import type { ProfileDto } from '@openrows/ipc';
import { needsConsent } from '../connections';
import type { Connections } from '../connections';
import styles from './Query.module.css';

type Status = ReturnType<Connections['statusOf']>;

/** What the bar says for a connection that is not up; `null` while it is. */
function message(profile: ProfileDto, status: Status): string | null {
  if (status.state === 'failed')
    return `Could not connect to ${profile.name}: ${status.message}`;
  if (status.state !== 'disconnected') return null;
  if (needsConsent(profile))
    return `${profile.name} is not connected. It asks before writes, so it was not reconnected automatically.`;
  return `${profile.name} is not connected.`;
}

/**
 * Restored (or dropped) tabs of a connection that is not up: why, and a
 * Reconnect button that runs the normal connect flow.
 */
export function ReconnectBar(props: {
  profile: ProfileDto;
  status: Status;
  onConnect: () => void;
}): React.JSX.Element | null {
  const text = message(props.profile, props.status);
  if (!text) return null;
  return (
    <div className={styles.reconnect} role="status">
      <span>{text}</span>
      <Button size="sm" onClick={props.onConnect}>
        {props.status.state === 'failed' ? 'Reconnect' : 'Connect'}
      </Button>
    </div>
  );
}

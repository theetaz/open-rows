// SPDX-License-Identifier: Apache-2.0
import { Button, StatusDot } from '@openrows/ui';
import type { ProfileDto } from '@openrows/ipc';
import { SidebarMenu } from '../../shell/SidebarMenu';
import { useSidebarMenu } from '../../shell/useSidebarMenu';
import { EnvironmentBadge } from './EnvironmentBadge';
import { statusLabel } from './sessions';
import type { SessionStatus } from './sessions';
import type { Connections } from './useConnections';
import styles from './ConnectionHeader.module.css';

const tones = {
  disconnected: 'neutral',
  connecting: 'info',
  connected: 'success',
  failed: 'danger',
} as const;

/** Active connection identity, live session status and connection actions. */
export function ConnectionHeader({
  connections,
}: {
  connections: Connections;
}): React.JSX.Element {
  const { active, problem } = connections;
  if (!active)
    return (
      <div className={styles.empty}>
        <p>
          {connections.profiles.length
            ? 'Choose a connection in the rail.'
            : 'No saved connections yet.'}
        </p>
        <Button
          size="sm"
          onClick={() => {
            connections.setSheet({ kind: 'new' });
          }}
        >
          New connection…
        </Button>
        {problem && <p className={styles.problem}>{problem}</p>}
      </div>
    );
  const status = connections.statusOf(active.id);
  return (
    <div className={styles.header}>
      <div className={styles.identity}>
        <strong title={active.name}>{active.name}</strong>
        <EnvironmentBadge env={active.env} />
        <ConnectionMenu
          profile={active}
          status={status}
          connections={connections}
        />
      </div>
      <div className={styles.status} role="status">
        <StatusDot tone={tones[status.state]} label={statusLabel(status)} />
      </div>
      {status.state === 'failed' && (
        <p className={styles.problem} role="alert">
          {status.message}
        </p>
      )}
      {problem && <p className={styles.problem}>{problem}</p>}
    </div>
  );
}

function ConnectionMenu(props: {
  profile: ProfileDto;
  status: SessionStatus;
  connections: Connections;
}): React.JSX.Element {
  const { opener, id, open, close, toggle } = useSidebarMenu();
  const { profile, connections } = props;
  const live =
    props.status.state === 'connected' || props.status.state === 'connecting';
  const actions: readonly (readonly [string, () => void])[] = [
    live
      ? ['Disconnect', connections.disconnect]
      : ['Connect', connections.connect],
    [
      'Connection settings…',
      () => {
        connections.setSheet({ kind: 'edit', profile });
      },
    ],
    ['Duplicate', () => void connections.duplicate(profile)],
    [
      'Delete…',
      () => {
        connections.confirmRemove(profile);
      },
    ],
  ];
  return (
    <>
      <button
        type="button"
        className={styles.menuButton}
        ref={opener}
        aria-label="Connection menu"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? id : undefined}
        onClick={toggle}
      >
        ⋯
      </button>
      {open && (
        <SidebarMenu
          id={id}
          label="Connection actions"
          opener={opener}
          onClose={close}
        >
          {actions.map(([name, run]) => (
            <button
              key={name}
              type="button"
              role="menuitem"
              onClick={() => {
                close();
                run();
              }}
            >
              {name}
            </button>
          ))}
        </SidebarMenu>
      )}
    </>
  );
}

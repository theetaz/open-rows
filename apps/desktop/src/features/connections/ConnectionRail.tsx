// SPDX-License-Identifier: Apache-2.0
import { Icon, IconButton } from '@openrows/ui';
import type { ProfileDto } from '@openrows/ipc';
import { workspaceGroups } from './workspaceGroups';
import { WorkspaceDisclosure } from './WorkspaceDisclosure';
import { DatabaseTiles } from './DatabaseTiles';
import { WorkspaceConnection } from './WorkspaceConnection';
import { useWorkspaceCollapse } from './useWorkspaceCollapse';
import type { OpenedDatabase, SessionStatus } from './sessions';
import styles from './Rail.module.css';

/** Opened databases of the active connection, shown under its tile. */
type RailDatabases = {
  databases?: readonly OpenedDatabase[];
  database?: string;
  onDatabase?: (name: string) => void;
  onCloseDatabase?: (name: string) => void;
  onProfileDatabase?: (id: string, name: string) => void;
  onCloseProfileDatabase?: (id: string, name: string) => void;
};

/**
 * A foldable server/database hierarchy with full names and environment accents.
 */
export function ConnectionRail(
  props: RailDatabases & {
    profiles: readonly ProfileDto[];
    activeId: string | undefined;
    statusOf: (profileId: string) => SessionStatus;
    onSelect: (profileId: string) => void;
    onNew: () => void;
    onGallery: () => void;
  },
): React.JSX.Element {
  const [compact, toggle] = useWorkspaceCollapse('rail');
  return (
    <nav className={styles.rail} aria-label="Workspaces" data-compact={compact}>
      <div className={styles.railHeading}>
        <span>Workspaces</span>
        <IconButton
          aria-label={
            compact ? 'Expand workspace sidebar' : 'Collapse workspace sidebar'
          }
          variant="ghost"
          onClick={toggle}
        >
          <span aria-hidden="true">{compact ? '»' : '«'}</span>
        </IconButton>
      </div>
      <WorkspaceGroups {...props} />
      <IconButton
        aria-label="New connection"
        variant="ghost"
        onClick={props.onNew}
      >
        <Icon name="plus" />
        <span className={styles.controlLabel}>New connection</span>
      </IconButton>
      <div className={styles.space} />
      <IconButton aria-label="API client" variant="ghost">
        API
      </IconButton>
      <IconButton
        aria-label="Component gallery"
        variant="ghost"
        onClick={props.onGallery}
      >
        <Icon name="settings" />
      </IconButton>
    </nav>
  );
}

type RailProps = Parameters<typeof ConnectionRail>[0];
function WorkspaceGroups(props: RailProps): React.JSX.Element {
  return (
    <div className={styles.workspaces}>
      {workspaceGroups(props.profiles).map((group) => (
        <WorkspaceDisclosure
          key={group.id}
          id={group.id}
          label={group.label}
          environment={group.environment}
          count={group.profiles.length}
        >
          {group.profiles.map((profile) => {
            const status = props.statusOf(profile.id);
            const selected = props.activeId === profile.id;
            const databases =
              status.state === 'connected' ? status.databases : [];
            const opened = props.onProfileDatabase
              ? databases
              : selected
                ? (props.databases ?? [])
                : [];
            return (
              <WorkspaceConnection
                key={profile.id}
                profile={profile}
                status={status}
                selected={selected}
                hasDatabases={opened.length > 0}
                onKeyDown={navigateRail}
                onSelect={() => {
                  props.onSelect(profile.id);
                }}
              >
                <DatabaseTiles
                  databases={opened}
                  active={selected ? (props.database ?? '') : ''}
                  onSelect={(name) => {
                    if (props.onProfileDatabase)
                      props.onProfileDatabase(profile.id, name);
                    else props.onDatabase?.(name);
                  }}
                  onClose={(name) => {
                    if (props.onCloseProfileDatabase)
                      props.onCloseProfileDatabase(profile.id, name);
                    else props.onCloseDatabase?.(name);
                  }}
                  onKeyDown={navigateRail}
                />
              </WorkspaceConnection>
            );
          })}
        </WorkspaceDisclosure>
      ))}
    </div>
  );
}

function navigateRail(event: React.KeyboardEvent<HTMLButtonElement>): void {
  if (event.nativeEvent.isComposing) return;
  if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return;
  const buttons = Array.from(
    event.currentTarget
      .closest('nav')
      ?.querySelectorAll<HTMLButtonElement>('[aria-pressed]') ?? [],
  );
  const index = buttons.indexOf(event.currentTarget);
  const next =
    (index + (event.key === 'ArrowDown' ? 1 : -1) + buttons.length) %
    buttons.length;
  event.preventDefault();
  buttons[next]?.focus();
}

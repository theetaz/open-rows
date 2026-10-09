// SPDX-License-Identifier: Apache-2.0
import { Icon, IconButton } from '@openrows/ui';
import type { ProfileDto } from '@openrows/ipc';
import { railOrder, railShortcut, workspaceGroups } from './workspaceGroups';
import { WorkspaceDisclosure } from './WorkspaceDisclosure';
import { DatabaseTiles } from './DatabaseTiles';
import { WorkspaceConnection } from './WorkspaceConnection';
import { useWorkspaceCollapse } from './useWorkspaceCollapse';
import type { ConnectionAction } from './connectionActions';
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
 * Full-height workspace sidebar: environment groups, one-line server rows and
 * their opened databases, with app-level actions at the bottom.
 */
export function ConnectionRail(
  props: RailDatabases & {
    profiles: readonly ProfileDto[];
    activeId: string | undefined;
    statusOf: (profileId: string) => SessionStatus;
    onSelect: (profileId: string) => void;
    onNew: () => void;
    actionsFor?: (profile: ProfileDto) => readonly ConnectionAction[];
    onSettings?: (() => void) | undefined;
  },
): React.JSX.Element {
  const [compact, toggle] = useWorkspaceCollapse('rail');
  return (
    <nav className={styles.rail} aria-label="Workspaces" data-compact={compact}>
      <div className={styles.railTop} data-tauri-drag-region />
      <div className={styles.railHeading}>
        <span>Connections</span>
        <IconButton
          aria-label="New connection"
          title="New connection"
          variant="ghost"
          size="sm"
          onClick={props.onNew}
        >
          <Icon name="plus" />
        </IconButton>
      </div>
      <WorkspaceGroups {...props} compact={compact} />
      <div className={styles.space} />
      <footer className={styles.railFooter}>
        <button type="button" className={styles.footerRow}>
          <Icon name="code" />
          <span className={styles.controlLabel}>API client</span>
        </button>
        {props.onSettings && (
          <button
            type="button"
            className={styles.footerRow}
            aria-label="Settings"
            onClick={props.onSettings}
          >
            <Icon name="settings" />
            <span className={styles.controlLabel}>Settings</span>
          </button>
        )}
        <IconButton
          aria-label={
            compact ? 'Expand workspace sidebar' : 'Collapse workspace sidebar'
          }
          title={compact ? 'Expand sidebar' : 'Collapse sidebar'}
          variant="ghost"
          size="sm"
          className={styles.collapse}
          onClick={toggle}
        >
          <Icon name={compact ? 'chevronRight' : 'chevronLeft'} />
        </IconButton>
      </footer>
    </nav>
  );
}

type RailProps = Parameters<typeof ConnectionRail>[0];
function WorkspaceGroups(
  props: RailProps & { compact: boolean },
): React.JSX.Element {
  const order = railOrder(props.profiles).map((profile) => profile.id);
  return (
    <div className={styles.workspaces}>
      {workspaceGroups(props.profiles).map((group) => (
        <WorkspaceDisclosure
          key={group.id}
          id={group.id}
          label={group.label}
          environment={group.environment}
          count={group.profiles.length}
          forceOpen={props.compact}
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
                actions={props.actionsFor?.(profile)}
                profile={profile}
                shortcut={railShortcut(order.indexOf(profile.id))}
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

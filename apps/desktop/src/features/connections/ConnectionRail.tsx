// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Icon, IconButton } from '@openrows/ui';
import type { ProfileDto } from '@openrows/ipc';
import { railOrder, railShortcut, workspaceGroups } from './workspaceGroups';
import { WorkspaceDisclosure } from './WorkspaceDisclosure';
import { DatabaseTiles } from './DatabaseTiles';
import { WorkspaceConnection } from './WorkspaceConnection';
import { useConnectionSearch } from './useConnectionSearch';
import { RailSearch } from './RailSearch';
import type { ConnectionAction } from './connectionActions';
import type { OpenedDatabase, SessionStatus } from './sessions';
import styles from './Rail.module.css';
import { t } from './messages/profile';

/** Opened databases of the active connection, shown under its tile. */
type RailDatabases = {
  onBackup?: ((id: string, name: string) => void) | undefined;
  onRestore?: ((id: string, name: string) => void) | undefined;
  databases?: readonly OpenedDatabase[];
  restoredDatabases?: ((id: string) => readonly string[]) | undefined;
  database?: string;
  onDatabase?: (name: string) => void;
  onCloseDatabase?: (name: string) => void;
  onProfileDatabase?: (id: string, name: string) => void;
  onCloseProfileDatabase?: (id: string, name: string) => void;
  /** Open a database's process list (engines with one only). */
  onProcesses?: ((id: string, name: string) => void) | undefined;
  /** Open the database Monitor; every engine has a collector with explicit source states. */
  onMonitor?: ((id: string, name: string) => void) | undefined;
  /** Run a `.sql` file on a database. */
  onRunFile?: ((id: string, name: string) => void) | undefined;
};

/**
 * The sidebar's connection list: environment groups, one-line server rows and
 * their opened databases, under a heading with the search field.
 */
export function ConnectionRail(
  props: RailDatabases & {
    profiles: readonly ProfileDto[];
    activeId: string | undefined;
    statusOf: (profileId: string) => SessionStatus;
    onSelect: (profileId: string) => void;
    onNew: () => void;
    actionsFor?: (profile: ProfileDto) => readonly ConnectionAction[];
  },
): React.JSX.Element {
  const [query, setQuery] = useState('');
  const matches = useConnectionSearch(query);
  const visible = matches
    ? props.profiles.filter((profile) => matches.has(profile.id))
    : props.profiles;
  return (
    <nav className={styles.rail} aria-label={t('workspaces')}>
      <div className={styles.railHeading}>
        <span>{t('connections')}</span>
        <IconButton
          aria-label={t('newConnection')}
          title={t('newConnection')}
          variant="ghost"
          size="sm"
          onClick={props.onNew}
        >
          <Icon name="plus" />
        </IconButton>
      </div>
      <RailSearch
        query={query}
        onQuery={setQuery}
        onSubmit={() => {
          const first = railOrder(visible)[0];
          if (first) props.onSelect(first.id);
        }}
      />
      {matches && visible.length === 0 ? (
        <p className={styles.noMatches} role="status">
          {t('noMatchingConnections')}
        </p>
      ) : (
        <WorkspaceGroups
          {...props}
          visible={visible}
          searching={matches !== null}
        />
      )}
    </nav>
  );
}

type RailProps = Parameters<typeof ConnectionRail>[0];
/** Groups of the visible (matching) connections; shortcuts follow the full list. */
function WorkspaceGroups(
  props: RailProps & {
    visible: readonly ProfileDto[];
    searching: boolean;
  },
): React.JSX.Element {
  const order = railOrder(props.profiles).map((profile) => profile.id);
  return (
    <div className={styles.workspaces}>
      {workspaceGroups(props.visible).map((group) => (
        <WorkspaceDisclosure
          key={group.id}
          id={group.id}
          label={group.label}
          environment={group.environment}
          count={group.profiles.length}
          forceOpen={props.searching}
        >
          {group.profiles.map((profile) => {
            const status = props.statusOf(profile.id);
            const selected = props.activeId === profile.id;
            const databases = restoredDatabases(props, profile.id, status);
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
                <RailDatabaseTiles
                  {...props}
                  profile={profile}
                  opened={opened}
                  selected={selected}
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

/** Live databases and restored disconnected placeholders share the same rail. */
function restoredDatabases(
  props: RailProps,
  id: string,
  status: SessionStatus,
): OpenedDatabase[] {
  const live = status.state === 'connected' ? status.databases : [];
  const disconnected: OpenedDatabase[] = (props.restoredDatabases?.(id) ?? [])
    .filter((name) => !live.some((item) => item.name === name))
    .map((name) => ({ name, state: 'disconnected' }));
  return [...live, ...disconnected];
}

function RailDatabaseTiles(
  props: Parameters<typeof WorkspaceGroups>[0] & {
    profile: ProfileDto;
    opened: readonly OpenedDatabase[];
    selected: boolean;
  },
): React.JSX.Element {
  const { profile, opened, selected } = props;
  return (
    <DatabaseTiles
      databases={opened}
      active={selected ? (props.database ?? '') : ''}
      onSelect={(name) => {
        if (props.onProfileDatabase) props.onProfileDatabase(profile.id, name);
        else props.onDatabase?.(name);
      }}
      onClose={(name) => {
        if (props.onCloseProfileDatabase)
          props.onCloseProfileDatabase(profile.id, name);
        else props.onCloseDatabase?.(name);
      }}
      onMonitor={
        props.onMonitor
          ? (name) => {
              props.onMonitor?.(profile.id, name);
            }
          : undefined
      }
      onProcesses={
        profile.processList && props.onProcesses
          ? (name) => {
              props.onProcesses?.(profile.id, name);
            }
          : undefined
      }
      onRunFile={
        props.onRunFile
          ? (name) => {
              props.onRunFile?.(profile.id, name);
            }
          : undefined
      }
      onBackup={
        props.onBackup
          ? (name) => {
              props.onBackup?.(profile.id, name);
            }
          : undefined
      }
      onRestore={
        props.onRestore
          ? (name) => {
              props.onRestore?.(profile.id, name);
            }
          : undefined
      }
      onKeyDown={navigateRail}
    />
  );
}

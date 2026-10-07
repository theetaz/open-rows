// SPDX-License-Identifier: Apache-2.0
import { Icon, IconButton, useReducedMotion } from '@openrows/ui';
import type { ProfileDto } from '@openrows/ipc';
import { DatabaseTiles } from './DatabaseTiles';
import { workspaceMonogram } from './monogram';
import { RailTooltip, useTileTooltip } from './RailTooltip';
import { statusLabel, statusTone } from './sessions';
import type { OpenedDatabase, SessionStatus } from './sessions';
import styles from './Rail.module.css';

/** Opened databases of the active connection, shown under its tile. */
type RailDatabases = {
  databases?: readonly OpenedDatabase[];
  database?: string;
  onDatabase?: (name: string) => void;
  onCloseDatabase?: (name: string) => void;
};

const ignore = (): void => undefined;

/**
 * Saved connections as monogram tiles with a connection-status dot, the
 * active connection's opened databases, then add, API and gallery controls.
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
  return (
    <nav className={styles.rail} aria-label="Workspaces">
      {props.profiles.map((profile) => (
        <WorkspaceTile
          key={profile.id}
          profile={profile}
          status={props.statusOf(profile.id)}
          selected={props.activeId === profile.id}
          onSelect={() => {
            props.onSelect(profile.id);
          }}
        >
          <DatabaseTiles
            databases={props.databases ?? []}
            active={props.database ?? ''}
            onSelect={props.onDatabase ?? ignore}
            onClose={props.onCloseDatabase ?? ignore}
            onKeyDown={navigateRail}
          />
        </WorkspaceTile>
      ))}
      <IconButton
        aria-label="New connection"
        variant="ghost"
        onClick={props.onNew}
      >
        <Icon name="plus" />
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

function WorkspaceTile(props: {
  profile: ProfileDto;
  status: SessionStatus;
  selected: boolean;
  onSelect: () => void;
  /** Opened database tiles, shown only for the selected connection. */
  children: React.ReactNode;
}): React.JSX.Element {
  const tooltip = useTileTooltip();
  const reduced = useReducedMotion();
  const { profile, selected } = props;
  return (
    <>
      <div className={styles.item}>
        <button
          type="button"
          className={styles.connection}
          aria-label={`Switch to ${profile.name}`}
          aria-pressed={selected}
          aria-describedby={tooltip.visible ? tooltip.id : undefined}
          data-session={props.status.state}
          onClick={props.onSelect}
          onKeyDown={navigateRail}
          {...tooltip.events}
        >
          <span
            aria-hidden="true"
            className={styles.indicator}
            data-state={
              selected ? 'selected' : tooltip.visible ? 'hover' : 'idle'
            }
            data-motion={reduced ? 'reduced' : 'full'}
          />
          <span className={styles.tile}>
            {workspaceMonogram(profile.name)}
            <i aria-hidden="true" data-status={statusTone(props.status)} />
          </span>
        </button>
        {tooltip.visible && (
          <RailTooltip
            profile={profile}
            status={statusLabel(props.status)}
            id={tooltip.id}
            position={tooltip.position}
          />
        )}
      </div>
      {selected && props.children}
    </>
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

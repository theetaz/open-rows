// SPDX-License-Identifier: Apache-2.0
import { Icon, IconButton, useReducedMotion } from '@openrows/ui';
import type { ProfileDto } from '@openrows/ipc';
import { customColour } from './EnvironmentBadge';
import { workspaceMonogram } from './monogram';
import { RailTooltip, useTileTooltip } from './RailTooltip';
import { statusLabel } from './sessions';
import type { SessionStatus } from './sessions';
import styles from './Rail.module.css';

/** Saved connections as monogram tiles, then add, API and gallery controls. */
export function ConnectionRail(props: {
  profiles: readonly ProfileDto[];
  activeId: string | undefined;
  statusOf: (profileId: string) => SessionStatus;
  onSelect: (profileId: string) => void;
  onNew: () => void;
  onGallery: () => void;
}): React.JSX.Element {
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
        />
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
}): React.JSX.Element {
  const tooltip = useTileTooltip();
  const reduced = useReducedMotion();
  const { profile, selected } = props;
  return (
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
          <i
            aria-hidden="true"
            data-environment={profile.env.kind}
            style={customColour(profile.env)}
          />
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

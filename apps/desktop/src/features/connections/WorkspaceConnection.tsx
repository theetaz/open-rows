// SPDX-License-Identifier: Apache-2.0
import type { ProfileDto } from '@openrows/ipc';
import { workspaceMonogram } from './monogram';
import { statusLabel, statusTone } from './sessions';
import type { SessionStatus } from './sessions';
import { RailTooltip, useTileTooltip } from './RailTooltip';
import { useWorkspaceCollapse } from './useWorkspaceCollapse';
import styles from './Rail.module.css';

/** Named server row with its database disclosure alongside the selection button. */
export function WorkspaceConnection(props: {
  profile: ProfileDto;
  status: SessionStatus;
  selected: boolean;
  onSelect: () => void;
  onKeyDown: (event: React.KeyboardEvent<HTMLButtonElement>) => void;
  hasDatabases: boolean;
  children: React.ReactNode;
}): React.JSX.Element {
  const { profile, status, selected } = props;
  const tooltip = useTileTooltip();
  const [collapsed, toggle] = useWorkspaceCollapse(profile.id);
  return (
    <div className={styles.server} data-environment={profile.env.kind}>
      <div className={styles.serverRow} data-selected={selected}>
        <button
          type="button"
          className={styles.serverDisclosure}
          aria-label={`${collapsed ? 'Expand' : 'Collapse'} ${profile.name} databases`}
          aria-expanded={!collapsed}
          disabled={!props.hasDatabases}
          onClick={toggle}
        >
          <span aria-hidden="true">{collapsed ? '›' : '⌄'}</span>
        </button>
        <button
          type="button"
          className={styles.connection}
          aria-label={`Switch to ${profile.name}`}
          aria-pressed={selected}
          aria-describedby={tooltip.visible ? tooltip.id : undefined}
          data-session={status.state}
          onClick={props.onSelect}
          onKeyDown={props.onKeyDown}
          {...tooltip.events}
        >
          <span className={styles.tile} aria-hidden="true">
            {workspaceMonogram(profile.name)}
            <i data-status={statusTone(status)} />
          </span>
          <span className={styles.serverName}>
            <strong>{profile.name}</strong>
            <small>{statusLabel(status)}</small>
          </span>
        </button>
      </div>
      {tooltip.visible && (
        <RailTooltip
          profile={profile}
          status={statusLabel(status)}
          id={tooltip.id}
          position={tooltip.position}
        />
      )}
      {!collapsed && props.children}
    </div>
  );
}

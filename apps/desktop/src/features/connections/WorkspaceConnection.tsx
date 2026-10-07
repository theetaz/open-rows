// SPDX-License-Identifier: Apache-2.0
import { useCallback, useId, useRef, useState } from 'react';
import { Icon } from '@openrows/ui';
import type { ProfileDto } from '@openrows/ipc';
import { SidebarMenu } from '../../shell/SidebarMenu';
import { workspaceMonogram } from './monogram';
import { statusLabel, statusTone } from './sessions';
import type { SessionStatus } from './sessions';
import type { ConnectionAction } from './connectionActions';
import { RailTooltip, useTileTooltip } from './RailTooltip';
import { useWorkspaceCollapse } from './useWorkspaceCollapse';
import styles from './Rail.module.css';

/** One-line server row: disclosure, monogram, name, status dot and actions. */
export function WorkspaceConnection(props: {
  profile: ProfileDto;
  status: SessionStatus;
  selected: boolean;
  onSelect: () => void;
  onKeyDown: (event: React.KeyboardEvent<HTMLButtonElement>) => void;
  hasDatabases: boolean;
  actions?: readonly ConnectionAction[] | undefined;
  children: React.ReactNode;
}): React.JSX.Element {
  const { profile, status, selected } = props;
  const tooltip = useTileTooltip();
  const [menuOpen, setMenuOpen] = useState(false);
  const [collapsed, toggle] = useWorkspaceCollapse(profile.id);
  return (
    <div className={styles.server} data-environment={profile.env.kind}>
      <div
        className={styles.serverRow}
        data-selected={selected}
        onContextMenu={(event) => {
          if (!props.actions) return;
          event.preventDefault();
          setMenuOpen(true);
        }}
      >
        <button
          type="button"
          className={styles.serverDisclosure}
          aria-label={`${collapsed ? 'Expand' : 'Collapse'} ${profile.name} databases`}
          aria-expanded={!collapsed}
          disabled={!props.hasDatabases}
          onClick={toggle}
        >
          <Icon name={collapsed ? 'chevronRight' : 'chevronDown'} />
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
          </span>
          <span className={styles.serverName}>
            <strong>{profile.name}</strong>
            <small>{statusLabel(status)}</small>
          </span>
          <i className={styles.status} data-status={statusTone(status)} />
        </button>
        {props.actions && (
          <RowActions
            name={profile.name}
            actions={props.actions}
            open={menuOpen}
            onOpenChange={setMenuOpen}
          />
        )}
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

/** ⋯ button (shown on hover/focus) and the row's action menu. */
function RowActions(props: {
  name: string;
  actions: readonly ConnectionAction[];
  open: boolean;
  onOpenChange: (open: boolean) => void;
}): React.JSX.Element {
  const opener = useRef<HTMLButtonElement>(null);
  const id = useId();
  const { onOpenChange } = props;
  const close = useCallback(() => {
    onOpenChange(false);
    opener.current?.focus();
  }, [onOpenChange]);
  return (
    <>
      <button
        ref={opener}
        type="button"
        className={styles.rowMenu}
        aria-label={`${props.name} actions`}
        aria-haspopup="menu"
        aria-expanded={props.open}
        onClick={() => {
          onOpenChange(!props.open);
        }}
      >
        <Icon name="more" />
      </button>
      {props.open && (
        <SidebarMenu
          id={id}
          label={`${props.name} actions`}
          opener={opener}
          onClose={close}
        >
          {props.actions.map(([name, run]) => (
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

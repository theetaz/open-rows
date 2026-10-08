// SPDX-License-Identifier: Apache-2.0
import { Fragment, useCallback, useId, useRef, useState } from 'react';
import { Icon } from '@openrows/ui';
import type { ProfileDto } from '@openrows/ipc';
import { SidebarMenu } from '../../shell/SidebarMenu';
import { PointMenu } from '../../shell/PointMenu';
import { workspaceMonogram } from './monogram';
import { statusLabel, statusTone } from './sessions';
import type { SessionStatus } from './sessions';
import type { ConnectionAction } from './connectionActions';
import { RailTooltip, useTileTooltip } from './RailTooltip';
import { useWorkspaceCollapse } from './useWorkspaceCollapse';
import styles from './Rail.module.css';

/** Where the row's action menu is open: under ⋯, or at a right-click. */
type MenuAt = 'button' | { x: number; y: number } | null;

function useRowMenu(): [MenuAt, (menu: MenuAt) => void, () => void] {
  const [menu, setMenu] = useState<MenuAt>(null);
  const close = useCallback(() => {
    setMenu(null);
  }, []);
  return [menu, setMenu, close];
}

/** One-line server row: disclosure, monogram with presence, name and actions. */
export function WorkspaceConnection(props: {
  profile: ProfileDto;
  /** ⌘ shortcut that switches to this connection. */
  shortcut?: string | undefined;
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
  const [menu, setMenu, closePoint] = useRowMenu();
  const [collapsed, toggle] = useWorkspaceCollapse(profile.id);
  return (
    <div className={styles.server} data-environment={profile.env.kind}>
      <div
        className={styles.serverRow}
        data-selected={selected}
        onContextMenu={(event) => {
          if (!props.actions) return;
          event.preventDefault();
          setMenu({ x: event.clientX, y: event.clientY });
        }}
      >
        <DatabasesDisclosure
          name={profile.name}
          collapsed={collapsed}
          disabled={!props.hasDatabases}
          onToggle={toggle}
        />
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
          <ServerLabel profile={profile} status={status} />
        </button>
        {props.actions && (
          <RowActions
            name={profile.name}
            actions={props.actions}
            open={menu === 'button'}
            onOpenChange={(open) => {
              setMenu(open ? 'button' : null);
            }}
          />
        )}
      </div>
      {props.actions && menu && menu !== 'button' && (
        <PointMenu
          {...menu}
          label={`${profile.name} actions`}
          onClose={closePoint}
        >
          <ActionItems actions={props.actions} onClose={closePoint} />
        </PointMenu>
      )}
      {tooltip.visible && !menu && (
        <RailTooltip
          profile={profile}
          shortcut={props.shortcut}
          status={statusLabel(status)}
          id={tooltip.id}
          position={tooltip.position}
        />
      )}
      {!collapsed && props.children}
    </div>
  );
}

/** Chevron that folds the connection's opened databases. */
function DatabasesDisclosure(props: {
  name: string;
  collapsed: boolean;
  disabled: boolean;
  onToggle: () => void;
}): React.JSX.Element {
  return (
    <button
      type="button"
      className={styles.serverDisclosure}
      aria-label={`${props.collapsed ? 'Expand' : 'Collapse'} ${props.name} databases`}
      aria-expanded={!props.collapsed}
      disabled={props.disabled}
      onClick={props.onToggle}
    >
      <Icon name={props.collapsed ? 'chevronRight' : 'chevronDown'} />
    </button>
  );
}

/** Monogram tile with its presence dot, then the name and status text. */
function ServerLabel(props: {
  profile: ProfileDto;
  status: SessionStatus;
}): React.JSX.Element {
  return (
    <>
      <span className={styles.tile} aria-hidden="true">
        {workspaceMonogram(props.profile.name)}
        <i className={styles.status} data-status={statusTone(props.status)} />
      </span>
      <span className={styles.serverName}>
        <strong>{props.profile.name}</strong>
        <small>{statusLabel(props.status)}</small>
      </span>
    </>
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
          <ActionItems actions={props.actions} onClose={close} />
        </SidebarMenu>
      )}
    </>
  );
}

function ActionItems(props: {
  actions: readonly ConnectionAction[];
  onClose: () => void;
}): React.JSX.Element {
  return (
    <>
      {props.actions.map(([name, run]) => {
        const danger = name.startsWith('Delete');
        return (
          <Fragment key={name}>
            {danger && <hr />}
            <button
              type="button"
              role="menuitem"
              data-danger={danger || undefined}
              onClick={() => {
                props.onClose();
                run();
              }}
            >
              {name}
            </button>
          </Fragment>
        );
      })}
    </>
  );
}

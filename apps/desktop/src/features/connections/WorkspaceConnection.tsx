// SPDX-License-Identifier: Apache-2.0
import { useCallback, useState } from 'react';
import { EngineIcon, Icon, isEngineId } from '@openrows/ui';
import type { IconName, MenuEntry } from '@openrows/ui';
import type { ProfileDto } from '@openrows/ipc';
import { ContextMenu } from '../../shell/ContextMenu';
import { useMenuButton } from '../../shell/useMenuButton';
import { workspaceMonogram } from './monogram';
import { statusLabel, statusTone } from './sessions';
import type { SessionStatus } from './sessions';
import type { ConnectionAction } from './connectionActions';
import { profileLocation } from './engines';
import { insecureDetail, insecureReasons } from './insecure';
import { InsecureBadge } from './InsecureBadge';
import { useWorkspaceCollapse } from './useWorkspaceCollapse';
import styles from './Rail.module.css';
import { t } from './messages/profile';

/** Where the row's context menu is open: at a right-click. */
type MenuAt = { x: number; y: number; target: Element } | null;

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
          setMenu({
            x: event.clientX,
            y: event.clientY,
            target: event.currentTarget,
          });
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
          aria-label={t('switchName', { name: profile.name })}
          aria-pressed={selected}
          title={rowTitle(profile, status, props.shortcut)}
          data-session={status.state}
          onClick={props.onSelect}
          onKeyDown={props.onKeyDown}
        >
          <ServerLabel profile={profile} status={status} />
        </button>
        {props.actions && (
          <RowActions name={profile.name} actions={props.actions} />
        )}
      </div>
      {props.actions && menu && (
        <ContextMenu
          label={t('nameActions', { name: profile.name })}
          at={menu}
          target={menu.target}
          entries={actionEntries(props.actions)}
          onClose={closePoint}
        />
      )}
      {!collapsed && props.children}
    </div>
  );
}

/** The row's native tooltip: name and shortcut, address, status, security. */
function rowTitle(
  profile: ProfileDto,
  status: SessionStatus,
  shortcut: string | undefined,
): string {
  const insecure = insecureReasons(profile, status);
  return [
    shortcut ? `${profile.name}  ${shortcut}` : profile.name,
    profileLocation(profile),
    statusLabel(status),
    insecure.length > 0
      ? t('notSecureInsecure', { insecure: insecureDetail(insecure) })
      : '',
  ]
    .filter(Boolean)
    .join('\n');
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
      aria-label={
        props.collapsed
          ? t('expandNameDatabases', { name: props.name })
          : t('collapseNameDatabases', { name: props.name })
      }
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
      <span
        className={styles.tile}
        data-engine={isEngineId(props.profile.engine) || undefined}
        aria-hidden="true"
      >
        {isEngineId(props.profile.engine) ? (
          <EngineIcon engine={props.profile.engine} size="sm" />
        ) : (
          workspaceMonogram(props.profile.name)
        )}
        <i className={styles.status} data-status={statusTone(props.status)} />
      </span>
      <span className={styles.serverName}>
        <strong>{props.profile.name}</strong>
        <small>{statusLabel(props.status)}</small>
      </span>
      <span className={styles.insecure}>
        <InsecureBadge
          reasons={insecureReasons(props.profile, props.status)}
          compact
        />
      </span>
    </>
  );
}

/** ⋯ button (shown on hover/focus) and the row's action menu. */
function RowActions(props: {
  name: string;
  actions: readonly ConnectionAction[];
}): React.JSX.Element {
  const { anchor, close, button } = useMenuButton();
  return (
    <>
      <button
        type="button"
        className={styles.rowMenu}
        aria-label={t('nameActions', { name: props.name })}
        {...button}
      >
        <Icon name="more" />
      </button>
      {anchor && (
        <ContextMenu
          label={t('nameActions', { name: props.name })}
          anchor={anchor}
          align="end"
          target={anchor.parentElement}
          entries={actionEntries(props.actions)}
          onClose={close}
        />
      )}
    </>
  );
}

/** Icon and group of each connection command, by its label. */
const actionStyle = new Map<string, [IconName, number]>([
  [t('connect'), ['plug', 0]],
  [t('disconnect'), ['power', 0]],
  [t('cancelConnection'), ['close', 0]],
  [t('revealFinderFile'), ['external', 1]],
  [t('copyLink'), ['link', 1]],
  [t('connectionSettings'), ['settings', 2]],
  [t('duplicate'), ['duplicate', 2]],
  [t('exportConnection'), ['upload', 3]],
  [t('exportGroup'), ['upload', 3]],
  [t('exportAllConnections'), ['upload', 3]],
  [t('importConnections'), ['download', 3]],
  [t('delete'), ['trash', 4]],
]);

/** The connection's commands, grouped, with Delete… last and red. */
function actionEntries(actions: readonly ConnectionAction[]): MenuEntry[] {
  const style = (label: string): [IconName, number] =>
    actionStyle.get(label) ?? ['more', 3];
  const sorted = [...actions].sort(
    ([left], [right]) => style(left)[1] - style(right)[1],
  );
  const entries: MenuEntry[] = [];
  let group = -1;
  for (const [label, run] of sorted) {
    const [icon, next] = style(label);
    if (group >= 0 && next !== group) entries.push({ kind: 'separator' });
    group = next;
    entries.push({
      kind: 'item',
      label,
      icon: <Icon name={icon} />,
      danger: label === t('delete'),
      run,
    });
  }
  return entries;
}

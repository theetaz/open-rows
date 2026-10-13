// SPDX-License-Identifier: Apache-2.0
import { Icon, IconButton, ToolbarGroup } from '@openrows/ui';
import type { IconName } from '@openrows/ui';
import type { ContextSegments } from './contextBar';
import { ConnectionStatus } from './ConnectionStatus';
import { InsecureBadge } from '../connections';
import { JobsButton } from '../jobs';
import { FavouriteButton } from '../schema/FavouriteButton';
import type { SidebarObject } from '../schema/sidebarIdentity';
import styles from './Workspace.module.css';
import { useWindowActive } from './useWindowActive';

type Props = {
  documents?: React.ReactNode;
  context: ContextSegments;
  /** The active tab's object, starred from the end of the context bar. */
  favourite?: SidebarObject | undefined;
  /** Connection state for the context bar's health dot. */
  health: string;
  onDatabase?: (() => void) | undefined;
  onPalette: () => void;
  onRefresh?: (() => void) | undefined;
  safe: boolean;
  prod: boolean;
  onSafe: () => void;
  sidebar: boolean;
  inspector: boolean;
  onSidebar: () => void;
  onInspector: () => void;
  onQuery: () => void;
};
/** Overlay titlebar right of the workspace sidebar; the context bar flexes. */
export function WorkspaceToolbar(props: Props): React.JSX.Element {
  useWindowActive();
  return (
    <header
      className={styles.toolbar}
      data-tauri-drag-region
      aria-label="Window toolbar"
    >
      <ToolbarGroup aria-label="Sidebar controls">
        <ToolButton
          label="Toggle sidebar (⌃⌘S)"
          icon="sidebar"
          pressed={props.sidebar}
          onClick={props.onSidebar}
        />
      </ToolbarGroup>
      <ToolbarGroup aria-label="Navigation">
        <ToolButton label="Back" icon="chevronLeft" />
        <ToolButton label="Forward" icon="chevronRight" />
      </ToolbarGroup>
      <ToolbarGroup aria-label="Connection tools">
        <IconButton
          aria-label="Safe mode"
          title={props.safe ? 'Safe mode on' : 'Safe mode off'}
          className={styles.safe}
          aria-pressed={props.safe}
          aria-disabled={props.prod}
          variant="ghost"
          size="sm"
          onClick={props.onSafe}
        >
          <Icon name={props.safe ? 'lock' : 'unlock'} />
        </IconButton>
        <ToolButton
          label="Open database (⌘K)"
          icon="database"
          onClick={props.onDatabase}
        />
        <ToolButton
          label="New SQL query"
          icon="terminal"
          onClick={props.onQuery}
        />
      </ToolbarGroup>
      <Context
        context={props.context}
        health={props.health}
        favourite={props.favourite}
        onDatabase={props.onDatabase}
      />
      <ToolbarGroup aria-label="Workspace actions">
        <ToolButton label="Refresh" icon="refresh" onClick={props.onRefresh} />
        <JobsButton />
        {props.documents}
        <ToolButton
          label="Command palette"
          icon="command"
          onClick={props.onPalette}
        />
      </ToolbarGroup>
      <ToolbarGroup aria-label="Inspector controls">
        <ToolButton
          label="Toggle inspector (⌥⌘I)"
          icon="inspector"
          pressed={props.inspector}
          onClick={props.onInspector}
        />
      </ToolbarGroup>
    </header>
  );
}
/** Icon-only toolbar action; the label doubles as its tooltip. */
function ToolButton(props: {
  label: string;
  icon: IconName;
  pressed?: boolean;
  onClick?: (() => void) | undefined;
}): React.JSX.Element {
  return (
    <IconButton
      aria-label={props.label}
      title={
        props.onClick
          ? props.label
          : `${props.label} is not available for this connection.`
      }
      disabled={!props.onClick}
      aria-pressed={props.pressed}
      variant="ghost"
      size="sm"
      onClick={props.onClick}
    >
      <Icon name={props.icon} />
    </IconButton>
  );
}
/** `ENV | PostgreSQL 14.17 : connection : database : schema.object`. */
function Context({
  context,
  health,
  favourite,
  onDatabase,
}: Pick<
  Props,
  'context' | 'health' | 'favourite' | 'onDatabase'
>): React.JSX.Element {
  const { env, engine, name, database, object } = context;
  const address = [engine, name, database, object].filter(Boolean).join(' : ');
  const title = env ? `${env} | ${address}` : address;
  return (
    <div
      className={styles.context}
      data-glass-rim="address"
      aria-label="Connection context"
      title={title}
    >
      <ConnectionStatus health={health} route={context.route} />
      {env && (
        <span className={styles.env}>
          {env}
          <span className={styles.bar} aria-hidden="true">
            {' | '}
          </span>
        </span>
      )}
      <InsecureBadge reasons={context.insecure} />
      <span className={styles.address}>
        <span>
          {engine} : {name}
        </span>
        {name !== 'No connection' && (
          <>
            <span aria-hidden="true">{' : '}</span>
            <button
              className={styles.databaseSegment}
              type="button"
              aria-label={`Open database (${database || 'none'})`}
              onClick={onDatabase}
              disabled={!onDatabase}
            >
              {database || 'No database'}
            </button>
          </>
        )}
        {object && <span>{` : ${object}`}</span>}
      </span>
      {favourite && (
        <FavouriteButton object={favourite} className={styles.star} />
      )}
    </div>
  );
}

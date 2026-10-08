// SPDX-License-Identifier: Apache-2.0
import { Icon, IconButton, ToolbarGroup } from '@openrows/ui';
import type { IconName } from '@openrows/ui';
import type { ContextSegments } from './contextBar';
import styles from './Workspace.module.css';

type Props = {
  context: ContextSegments;
  /** Connection state for the context bar's health dot. */
  health: string;
  onDatabase: () => void;
  onPalette: () => void;
  onRefresh: () => void;
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
        onDatabase={props.onDatabase}
      />
      <ToolbarGroup aria-label="Workspace actions">
        <ToolButton label="Refresh" icon="refresh" onClick={props.onRefresh} />
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
  onClick?: () => void;
}): React.JSX.Element {
  return (
    <IconButton
      aria-label={props.label}
      title={props.label}
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
  onDatabase,
}: Pick<Props, 'context' | 'health' | 'onDatabase'>): React.JSX.Element {
  const { env, engine, name, database, object } = context;
  const address = [engine, name, database, object].filter(Boolean).join(' : ');
  const title = env ? `${env} | ${address}` : address;
  return (
    <div
      className={styles.context}
      aria-label="Connection context"
      title={title}
    >
      <span aria-hidden="true" className={styles.health} data-state={health}>
        ●
      </span>
      {env && (
        <span className={styles.env}>
          {env}
          <span className={styles.bar} aria-hidden="true">
            {' | '}
          </span>
        </span>
      )}
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
            >
              {database || 'No database'}
            </button>
          </>
        )}
        {object && <span>{` : ${object}`}</span>}
      </span>
    </div>
  );
}

// SPDX-License-Identifier: Apache-2.0
import { Button, Icon, IconButton, ToolbarGroup } from '@openrows/ui';
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
/** Full-width overlay titlebar; context yields space before any fixed control. */
export function WorkspaceToolbar(props: Props): React.JSX.Element {
  return (
    <header
      className={styles.toolbar}
      data-tauri-drag-region
      aria-label="Window toolbar"
    >
      <ToolbarGroup aria-label="Sidebar controls">
        <IconButton
          aria-label="Toggle sidebar (⌃⌘S)"
          aria-pressed={props.sidebar}
          variant="ghost"
          size="sm"
          onClick={props.onSidebar}
        >
          <Icon name="sidebar" />
        </IconButton>
      </ToolbarGroup>
      <ToolbarGroup aria-label="Navigation">
        <IconButton aria-label="Back" variant="ghost" size="sm">
          ‹
        </IconButton>
        <IconButton aria-label="Forward" variant="ghost" size="sm">
          ›
        </IconButton>
      </ToolbarGroup>
      <Button
        size="sm"
        className={styles.safe}
        aria-label="Safe mode"
        aria-pressed={props.safe}
        aria-disabled={props.prod}
        onClick={props.onSafe}
      >
        <span aria-hidden="true">♢</span>{' '}
        {props.safe ? 'Safe mode' : 'Safe mode off'}
      </Button>
      <Context
        context={props.context}
        health={props.health}
        onDatabase={props.onDatabase}
      />
      <WorkspaceActions {...props} />
      <ToolbarGroup aria-label="Inspector controls">
        <IconButton
          aria-label="Toggle inspector (⌥⌘I)"
          aria-pressed={props.inspector}
          variant="ghost"
          size="sm"
          onClick={props.onInspector}
        >
          <Icon name="inspector" />
        </IconButton>
      </ToolbarGroup>
    </header>
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

function WorkspaceActions(props: Props): React.JSX.Element {
  return (
    <ToolbarGroup aria-label="Workspace actions">
      <IconButton
        aria-label="Refresh"
        onClick={props.onRefresh}
        variant="ghost"
        size="sm"
      >
        <Icon name="refresh" />
      </IconButton>
      <Button
        aria-label="New SQL query"
        variant="ghost"
        size="sm"
        onClick={props.onQuery}
      >
        SQL
      </Button>
      <IconButton
        aria-label="Command palette"
        onClick={props.onPalette}
        variant="ghost"
        size="sm"
      >
        ⌘
      </IconButton>
    </ToolbarGroup>
  );
}

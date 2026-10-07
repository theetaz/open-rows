// SPDX-License-Identifier: Apache-2.0
import { Button, Icon, IconButton, ToolbarGroup } from '@openrows/ui';
import { EnvironmentBadge, statusLabel } from '../features/connections';
import type { Connections } from '../features/connections';
import styles from './Workspace.module.css';

type Props = {
  connections: Connections;
  table: string;
  database: string;
  onDatabase: () => void;
  onPalette: () => void;
  onRefresh: () => void;
  schema: string;
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
        connections={props.connections}
        table={props.table}
        schema={props.schema}
        database={props.database}
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
function Context({
  connections,
  table,
  schema,
  database,
  onDatabase,
}: Pick<
  Props,
  'connections' | 'table' | 'schema' | 'database' | 'onDatabase'
>): React.JSX.Element {
  const { active } = connections;
  const name = active?.name ?? 'No connection';
  const status = active ? connections.statusOf(active.id) : undefined;
  const engine = status ? statusLabel(status) : 'Not connected';
  const tail = table ? ` › ${table}` : '';
  const address = `${name} › ${database} › ${schema}${tail}`;
  return (
    <div className={styles.context} aria-label="Connection context">
      {active && <EnvironmentBadge env={active.env} />}
      <span className={styles.address} title={address}>
        <span>{name} › </span>
        <button
          className={styles.databaseSegment}
          type="button"
          aria-label={`Switch database (${database})`}
          onClick={onDatabase}
        >
          {database}
        </button>
        <span>
          {' '}
          › {schema}
          {tail}
        </span>
      </span>
      <span className={styles.engine} title={engine}>
        <span
          aria-hidden="true"
          className={styles.health}
          data-state={status?.state ?? 'disconnected'}
        >
          ●
        </span>{' '}
        {engine}
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

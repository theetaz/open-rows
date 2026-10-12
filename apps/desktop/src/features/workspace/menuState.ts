// SPDX-License-Identifier: Apache-2.0
import type { Connections } from '../connections';
import { OBJECT_COMMANDS, objectCommandSupported } from '../objects';
import type { ObjectCommand } from '../objects';
import type { TabScope } from '../query';

/**
 * Menu items that depend on app state (commands not listed stay enabled):
 * tab items need a tab, query items a query tab, connection items the
 * active connection's state.
 */
export function menuEnabled(input: {
  connections: Connections;
  tab:
    | { kind: 'query' | 'table' | 'processes' | 'monitor' | 'inspection' }
    | undefined;
  scope: TabScope | undefined;
  prod: boolean;
}): Record<string, boolean> {
  const profile = input.connections.active;
  const state = profile
    ? input.connections.statusOf(profile.id).state
    : 'disconnected';
  const connected = state === 'connected';
  const full = profile?.engine !== 'duckdb';
  const query = input.tab?.kind === 'query';
  return {
    ...tableEnabled(input.tab?.kind === 'table', connected),
    'new-tab': Boolean(input.scope),
    'close-tab': Boolean(input.tab),
    'open-file': Boolean(input.scope),
    save: query,
    'save-as': query,
    ...backgroundEnabled({
      full,
      connected,
      tab: input.tab,
      scope: input.scope,
    }),
    ...queryEnabled(query, connected, full),
    'split-right': Boolean(input.tab),
    'split-down': Boolean(input.tab),
    'focus-pane': Boolean(input.tab),
    'close-split': Boolean(input.tab),
    ...connectionEnabled(input, state),
    ...objectEnabled(input, connected),
  };
}

/** Commands on the active table tab's relation (the rest need a database). */
const relationCommands = new Set<ObjectCommand>([
  'new-trigger',
  'rename-table',
  'duplicate-table',
  'truncate-table',
  'drop-table',
  'edit-view',
  'drop-view',
]);

/** Object editors follow the engine's support and what is open. */
function objectEnabled(
  input: {
    connections: Connections;
    tab:
      | { kind: 'query' | 'table' | 'processes' | 'monitor' | 'inspection' }
      | undefined;
    scope: TabScope | undefined;
  },
  connected: boolean,
): Record<string, boolean> {
  const features = input.connections.active?.objects;
  const table = input.tab?.kind === 'table';
  return Object.fromEntries(
    OBJECT_COMMANDS.map((id) => [
      id,
      connected &&
        Boolean(features && objectCommandSupported(id, features)) &&
        (relationCommands.has(id) ? table : Boolean(input.scope)),
    ]),
  );
}

/** Connection items follow the active connection's state and capabilities. */
function connectionEnabled(
  input: { connections: Connections; prod: boolean },
  state: string,
): Record<string, boolean> {
  const profile = input.connections.active;
  const connected = state === 'connected';
  return {
    connect: Boolean(profile) && !connected && state !== 'connecting',
    disconnect: connected || state === 'connecting',
    database: connected,
    monitor: connected,
    processes: connected && Boolean(profile?.processList),
    refresh: connected,
    quick: connected,
    'show-er-diagram': connected,
    safe: Boolean(profile) && !input.prod,
  };
}

/** Query items need a query tab; those that run SQL, a session. */
function queryEnabled(
  query: boolean,
  connected: boolean,
  full: boolean,
): Record<string, boolean> {
  const runs = query && connected;
  return {
    'run-current': runs,
    'run-all': runs,
    cancel: runs,
    explain: full && runs,
    'explain-analyze': full && runs,
    beautify: query,
    uglify: query,
    'insert-snippet': query,
    'run-no-limit': runs,
  };
}

/** Table and edit commands need a table tab; those that touch rows, a session. */
function tableEnabled(
  table: boolean,
  connected: boolean,
): Record<string, boolean> {
  const rows = table && connected;
  return {
    'commit-changes': rows,
    'discard-changes': table,
    'insert-row': rows,
    'delete-row': rows,
    'copy-selection': rows,
    'focus-filter': table,
    'toggle-columns': table,
  };
}

/** DuckDB exports queue on its one session; its import, file runs and backups are pending. */
function backgroundEnabled(input: {
  full: boolean;
  connected: boolean;
  tab: { kind: string } | undefined;
  scope: TabScope | undefined;
}): Record<string, boolean> {
  const { full, connected } = input;
  const exportable = input.tab?.kind === 'query' || input.tab?.kind === 'table';
  const ready = full && connected;
  return {
    'export-table': connected && input.tab?.kind === 'table',
    'import-data': ready && Boolean(input.scope),
    'export-tables': connected,
    'run-sql-file': ready,
    'backup-database': ready,
    'restore-database': ready,
    'backup-schema': ready,
    'restore-schema': ready,
    'backup-table': ready && input.tab?.kind === 'table',
    'export-results': connected && exportable,
  };
}

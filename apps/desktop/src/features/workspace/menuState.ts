// SPDX-License-Identifier: Apache-2.0
import type { Connections } from '../connections';
import type { TabScope } from '../query';

/**
 * Menu items that depend on app state (commands not listed stay enabled):
 * tab items need a tab, query items a query tab, connection items the
 * active connection's state.
 */
export function menuEnabled(input: {
  connections: Connections;
  tab: { kind: 'query' | 'table' | 'processes' } | undefined;
  scope: TabScope | undefined;
  prod: boolean;
}): Record<string, boolean> {
  const profile = input.connections.active;
  const state = profile
    ? input.connections.statusOf(profile.id).state
    : 'disconnected';
  const connected = state === 'connected';
  const query = input.tab?.kind === 'query';
  const processes = input.tab?.kind === 'processes';
  return {
    ...tableEnabled(input.tab?.kind === 'table', connected),
    'new-tab': Boolean(input.scope),
    'close-tab': Boolean(input.tab),
    'open-file': Boolean(input.scope),
    save: query,
    'save-as': query,
    'export-table': input.tab?.kind === 'table' && connected,
    'export-results': Boolean(input.tab) && !processes && connected,
    'run-current': query && connected,
    'run-all': query && connected,
    cancel: query && connected,
    beautify: query,
    uglify: query,
    'insert-snippet': query,
    ...connectionEnabled(input, state),
  };
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
    processes: connected && Boolean(profile?.processList),
    refresh: connected,
    quick: connected,
    'show-er-diagram': connected,
    safe: Boolean(profile) && !input.prod,
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

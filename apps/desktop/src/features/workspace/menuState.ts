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
  tab: { kind: 'query' | 'table' } | undefined;
  scope: TabScope | undefined;
  prod: boolean;
}): Record<string, boolean> {
  const profile = input.connections.active;
  const state = profile
    ? input.connections.statusOf(profile.id).state
    : 'disconnected';
  const connected = state === 'connected';
  const query = input.tab?.kind === 'query';
  return {
    ...tableEnabled(input.tab?.kind === 'table', connected),
    'new-tab': Boolean(input.scope),
    'close-tab': Boolean(input.tab),
    'open-file': Boolean(input.scope),
    save: query,
    'save-as': query,
    'export-table': input.tab?.kind === 'table' && connected,
    'export-results': Boolean(input.tab) && connected,
    'run-current': query && connected,
    'run-all': query && connected,
    cancel: query && connected,
    beautify: query,
    uglify: query,
    connect: Boolean(profile) && !connected && state !== 'connecting',
    disconnect: connected || state === 'connecting',
    database: connected,
    refresh: connected,
    quick: connected,
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

// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { menuEnabled } from './menuState';
import type { Connections } from '../connections';
import { profile } from '../../test/ipcStub';
it('enables DuckDB browsing and export while gating its pending background features', () => {
  const active = profile('analytics', { engine: 'duckdb' });
  const connections = {
    active,
    statusOf: () => ({ state: 'connected' }),
  } as unknown as Connections;
  const enabled = menuEnabled({
    connections,
    tab: { kind: 'query' },
    scope: { connectionId: active.id, database: 'main' },
    prod: false,
  });
  for (const command of [
    'run-current',
    'run-all',
    'cancel',
    'new-tab',
    'export-results',
    'export-tables',
    'show-er-diagram',
    'database',
    'refresh',
    'quick',
  ])
    expect(enabled[command]).toBe(true);
  for (const command of [
    'explain',
    'explain-analyze',
    'backup-database',
    'restore-database',
    'import-data',
    'run-sql-file',
  ])
    expect(enabled[command]).toBe(false);
});

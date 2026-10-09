// SPDX-License-Identifier: Apache-2.0
import { vi } from 'vitest';
import type { ProfileDto, ServerDto, TableStructureDto } from '@openrows/ipc';
import { catalogStub } from './catalogStub';
import { structureStub } from './structureStub';

/** A saved profile with synthetic, non-secret values. */
export function profile(
  name: string,
  overrides: Partial<ProfileDto> = {},
): ProfileDto {
  return {
    id: `id-${name}`,
    name,
    group: null,
    env: { kind: 'local', label: null, color: '#6b7280' },
    engine: 'postgres',
    file: null,
    host: 'localhost',
    port: 5432,
    database: 'orders',
    user: 'app',
    tlsMode: 'prefer',
    tlsFiles: {
      caFile: null,
      cloudCa: null,
      clientCert: null,
      clientKey: null,
      hasKeyPassphrase: false,
      unverifiedRemote: false,
    },
    ssh: null,
    proxy: null,
    hasPassword: false,
    safety: { readOnly: false, confirmWrites: false },
    insecure: [],
    recordHistory: true,
    ...overrides,
  };
}

/** Two local profiles and one prod profile for workspace tests. */
export const sampleProfiles: readonly ProfileDto[] = [
  profile('orders-local'),
  profile('analytics-prod', {
    env: { kind: 'prod', label: null, color: '#dc2626' },
    safety: { readOnly: false, confirmWrites: true },
  }),
  profile('events-dev', {
    env: { kind: 'dev', label: null, color: '#16a34a' },
  }),
];

/** What a synthetic PostgreSQL server reports after connecting. */
export function server(serverVersion: string, database = 'orders'): ServerDto {
  return { product: 'PostgreSQL', serverVersion, flavour: null, database };
}

/** A synthetic table structure: `id` (primary key) and `name`. */
const sampleStructure: TableStructureDto = {
  columns: [
    {
      name: 'id',
      dataType: 'integer',
      nullable: false,
      default: null,
      comment: null,
      primaryKey: true,
      enumLabels: null,
    },
    {
      name: 'name',
      dataType: 'text',
      nullable: true,
      default: "'anon'::text",
      comment: 'Display name',
      primaryKey: false,
      enumLabels: null,
    },
  ],
  indexes: [
    {
      name: 'orders_pkey',
      columns: 'id',
      unique: true,
      primary: true,
      method: 'btree',
      predicate: null,
    },
  ],
  rowEstimate: 1811,
};

/** Saved queries and `.sql` files: one saved query, no native panels. */
function savedQueryStub(): Record<string, unknown> {
  return {
    listSavedQueries: vi.fn(() =>
      Promise.resolve([
        {
          id: 'saved-1',
          name: 'Top customers',
          sql: 'SELECT * FROM customers ORDER BY total DESC LIMIT 10;',
          createdAtMs: 0,
          updatedAtMs: 0,
        },
      ]),
    ),
    createSavedQuery: vi.fn(() => Promise.resolve()),
    renameSavedQuery: vi.fn(() => Promise.resolve()),
    deleteSavedQuery: vi.fn(() => Promise.resolve()),
    saveSqlFile: vi.fn(() => Promise.resolve()),
    saveSqlFileAs: vi.fn(() => Promise.resolve(null)),
    openSqlFile: vi.fn(() => Promise.resolve(null)),
  };
}

/** settings.json and the menu bar: no file yet, no native menu. */
function appStub(): Record<string, unknown> {
  return {
    readSettingsFile: vi.fn(() => Promise.resolve(null)),
    writeSettingsFile: vi.fn(() => Promise.resolve()),
    syncMenu: vi.fn(() => Promise.resolve([])),
    onMenuCommand: vi.fn(() => Promise.resolve(() => undefined)),
    crashRecoveryStatus: vi.fn(() => Promise.resolve(false)),
    openLogsFolder: vi.fn(() => Promise.resolve()),
    reportUiEvent: vi.fn(() => Promise.resolve()),
    journaledTableEdits: vi.fn(() => Promise.resolve([])),
  };
}

/** `@openrows/ipc` replacement for jsdom tests: no native shell, no keychain. */
export function ipcStub(
  profiles: readonly ProfileDto[] = sampleProfiles,
): Record<string, unknown> {
  return {
    DEFAULT_CUSTOM_ENV_COLOR: '#7c3aed',
    getAppInfo: vi.fn(() => new Promise(() => {})),
    listProfiles: vi.fn(() =>
      Promise.resolve({ profiles: [...profiles], unreadable: [] }),
    ),
    onSessionState: vi.fn(() => Promise.resolve(() => {})),
    openConnection: vi.fn((profileId: string) =>
      Promise.resolve({
        server: server('17.2', 'postgres'),
        database:
          profiles.find((item) => item.id === profileId)?.database ?? null,
        insecure: [],
      }),
    ),
    closeConnection: vi.fn(() => Promise.resolve()),
    openSession: vi.fn((profileId: string, database: string) =>
      Promise.resolve({
        sessionId: `session-${profileId}-${database}`,
        server: server('17.2', database),
      }),
    ),
    ...catalogStub(),
    closeSession: vi.fn(() => Promise.resolve()),
    parseConnectionUrl: vi.fn(),
    testConnection: vi.fn(),
    pickConnectionFile: vi.fn(() => Promise.resolve(null)),
    onSshPrompt: vi.fn(() => Promise.resolve(() => {})),
    answerSshPrompt: vi.fn(() => Promise.resolve()),
    createProfile: vi.fn(),
    updateProfile: vi.fn(),
    duplicateProfile: vi.fn(),
    deleteProfile: vi.fn(() => Promise.resolve()),
    confirmDestructive: vi.fn(() => Promise.resolve(null)),
    pickFileValue: vi.fn(() => Promise.resolve(null)),
    CommandError: class CommandError extends Error {},
    executeQuery: vi.fn(),
    cancelQuery: vi.fn(() => Promise.resolve()),
    releaseResults: vi.fn(() => Promise.resolve()),
    searchHistory: vi.fn(() => Promise.resolve([])),
    deleteHistory: vi.fn(() => Promise.resolve(true)),
    clearHistory: vi.fn(() => Promise.resolve(null)),
    resultCellInfo: vi.fn(),
    resultCellValue: vi.fn(),
    ...savedQueryStub(),
    tableEditInfo: vi.fn(() =>
      Promise.resolve({
        key: [],
        protected: [],
        columns: sampleStructure.columns,
        readOnly: false,
        confirmWrites: false,
        production: false,
      }),
    ),
    tableEditRow: vi.fn(() => Promise.resolve({ id: '7', name: '' })),
    tableEditRows: vi.fn((_target: unknown, keys: Record<string, string>[]) =>
      Promise.resolve(keys.map((key) => ({ ...key, name: '' }))),
    ),
    saveTableEdits: vi.fn(() => Promise.resolve(null)),
    savedTableEdits: vi.fn(() => Promise.resolve(null)),
    clearTableEdits: vi.fn(() => Promise.resolve(null)),
    ...appStub(),
    onWindowClose: vi.fn(() => Promise.resolve(() => undefined)),
    closeWindow: vi.fn(() => Promise.resolve()),
    tableEdit: vi.fn(() =>
      Promise.resolve({ kind: 'prepared', token: 'review', statements: [] }),
    ),
    tableStructure: vi.fn(() => Promise.resolve(sampleStructure)),
    ...structureStub(),
    tablePage: vi.fn(() => new Promise(() => {})),
    tableCount: vi.fn(() => new Promise(() => {})),
    tableSql: vi.fn(() => Promise.resolve({ sql: 'SELECT 1', params: [] })),
    startExport: vi.fn(() => Promise.resolve({ status: 'dismissed' })),
  };
}

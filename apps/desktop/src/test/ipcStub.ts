// SPDX-License-Identifier: Apache-2.0
import { vi } from 'vitest';
import type { ProfileDto, ServerDto, TableStructureDto } from '@openrows/ipc';
import { catalogStub } from './catalogStub';
import { dumpStub } from './dumpStub';
import { importStub } from './importStub';
import { structureStub } from './structureStub';

/** Every structure and object editor PostgreSQL offers (no character sets). */
export const postgresObjects: ProfileDto['objects'] = {
  tables: true,
  views: true,
  materializedViews: true,
  triggers: true,
  routines: true,
  databases: true,
  renameDatabases: true,
  schemas: true,
  foreignKeys: true,
  checks: true,
  comments: true,
  collations: true,
  charsets: false,
  identity: true,
  partialIndexes: true,
  keys: true,
  typeUsing: true,
  concurrentIndexes: true,
  columnReordering: false,
};

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
    helper: null,
    hasPassword: false,
    passwordStorage: 'keychain',
    credentialCommand: null,
    tags: [],
    safety: 'unrestricted',
    insecure: [],
    recordHistory: true,
    redactHistory: false,
    limits: { statementTimeoutSeconds: null, idleTimeoutMinutes: 0 },
    startupCommands: '',
    processList: true,
    objects: postgresObjects,
    ...overrides,
  };
}

/** Two local profiles and one prod profile for workspace tests. */
export const sampleProfiles: readonly ProfileDto[] = [
  profile('orders-local'),
  profile('analytics-prod', {
    env: { kind: 'prod', label: null, color: '#dc2626' },
    safety: 'confirmWrites',
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

/** The ER diagram: `orders` referencing `customers`; export is cancelled. */
function diagramStub(): Record<string, unknown> {
  const columns = sampleStructure.columns.map((column) => ({
    name: column.name,
    dataType: column.dataType,
    nullable: column.nullable,
    primaryKey: column.primaryKey,
  }));
  return {
    schemaDiagram: vi.fn(() =>
      Promise.resolve({
        tables: [
          { name: 'customers', columns },
          {
            name: 'orders',
            columns: [
              ...columns,
              {
                name: 'customer_id',
                dataType: 'integer',
                nullable: false,
                primaryKey: false,
              },
            ],
          },
        ],
        foreignKeys: [
          {
            name: 'orders_customer_id_fkey',
            table: 'orders',
            columns: ['customer_id'],
            targetTable: 'customers',
            targetColumns: ['id'],
          },
        ],
        truncated: false,
        limit: 400,
      }),
    ),
    saveDiagram: vi.fn(() => Promise.resolve(null)),
  };
}

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
    saveSqlFile: vi.fn(() => Promise.resolve(null)),
    sqlFileModified: vi.fn(() => Promise.resolve(null)),
    readSqlFile: vi.fn(() => Promise.reject(new Error('no file'))),
    saveSqlFileAs: vi.fn(() => Promise.resolve(null)),
    openSqlFile: vi.fn(() => Promise.resolve(null)),
  };
}

/** Projects: no native folder panel; every file action succeeds. */
function projectStub(): Record<string, unknown> {
  return {
    projectSqlItems: vi.fn(() => Promise.resolve([])),
    createProjectSnippet: vi.fn(() => Promise.resolve('snippets/example.sql')),
    shareProjectConnections: vi.fn(() => Promise.resolve()),
    chooseProject: vi.fn(() => Promise.resolve(null)),
    loadProject: vi.fn(() => Promise.reject(new Error('no project'))),
    setProjectConnection: vi.fn(),
    createProjectFolder: vi.fn(() => Promise.resolve('queries/new')),
    createProjectQuery: vi.fn(() => Promise.resolve('queries/new.sql')),
    renameProjectItem: vi.fn(() => Promise.resolve('queries/renamed.sql')),
    moveProjectItem: vi.fn(() => Promise.resolve('queries/moved.sql')),
    duplicateProjectItem: vi.fn(() => Promise.resolve('queries/copy.sql')),
    deleteProjectItem: vi.fn(() => Promise.resolve()),
    revealProjectItem: vi.fn(() => Promise.resolve()),
  };
}

/** Key of a part of the stubbed local-store workspace state in web storage, cleared before each test. */
export const stubStateKey = (part: string): string => `test.workspace.${part}`;

/** Workspace state in an in-memory local store (web storage under a test key). */
function workspaceStub(): Record<string, unknown> {
  return {
    readWorkspaceState: vi.fn((part: string) =>
      Promise.resolve(localStorage.getItem(stubStateKey(part))),
    ),
    writeWorkspaceState: vi.fn((part: string, state: string | null) => {
      if (state === null) localStorage.removeItem(stubStateKey(part));
      else localStorage.setItem(stubStateKey(part), state);
      return Promise.resolve();
    }),
  };
}

/** settings.json and the menu bar: no file yet, no native menu. */
function appStub(): Record<string, unknown> {
  return {
    readSettingsFile: vi.fn(() => Promise.resolve(null)),
    writeSettingsFile: vi.fn(() => Promise.resolve()),
    syncMenu: vi.fn(() => Promise.resolve([])),
    onMenuCommand: vi.fn(() => Promise.resolve(() => undefined)),
    onSettingsFileChanged: vi.fn(() => Promise.resolve(() => undefined)),
    crashRecoveryStatus: vi.fn(() => Promise.resolve(false)),
    openLogsFolder: vi.fn(() => Promise.resolve()),
    reportUiEvent: vi.fn(() => Promise.resolve()),
    journaledTableEdits: vi.fn(() => Promise.resolve([])),
  };
}

/** Connection search over name, host and environment, and a revealed secret. */
function profileSearchStub(
  profiles: readonly ProfileDto[],
): Record<string, unknown> {
  return {
    searchProfiles: vi.fn((query: string) =>
      Promise.resolve(
        profiles.filter((item) =>
          `${item.name} ${item.host} ${item.env.kind}`.includes(query),
        ),
      ),
    ),
    revealSecret: vi.fn(() => Promise.resolve('pw-revealed-synthetic')),
  };
}

/** Table editing: a writable table keyed by nothing, with no saved edits. */
function tableEditStub(): Record<string, unknown> {
  return {
    tableEditInfo: vi.fn(() =>
      Promise.resolve({
        key: [],
        protected: [],
        rowId: null,
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
  };
}

/** `@openrows/ipc` replacement for jsdom tests: no native shell, no keychain. */
export function ipcStub(
  profiles: readonly ProfileDto[] = sampleProfiles,
): Record<string, unknown> {
  return {
    ...mcpStub(),
    ...appParityStub(),

    ...connectionTransferStub(),
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
    ...profileSearchStub(profiles),
    openSession: vi.fn((profileId: string, database: string) =>
      Promise.resolve({
        sessionId: `session-${profileId}-${database}`,
        server: server('17.2', database),
      }),
    ),
    ...catalogStub(),
    ...diagramStub(),
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
    explainQuery: vi.fn(() => new Promise(() => {})),
    releaseResults: vi.fn(() => Promise.resolve()),
    searchHistory: vi.fn(() => Promise.resolve([])),
    deleteHistory: vi.fn(() => Promise.resolve(true)),
    historyLiterals: vi.fn(() => Promise.resolve([])),
    clearHistory: vi.fn(() => Promise.resolve(null)),
    resultCellInfo: vi.fn(),
    resultCellValue: vi.fn(),
    ...savedQueryStub(),
    ...projectStub(),
    ...tableEditStub(),
    ...appStub(),
    ...workspaceStub(),
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
    ...importStub(),
    revealExport: vi.fn(() => Promise.resolve()),
    openExport: vi.fn(() => Promise.resolve()),
    pickRunFile: vi.fn(() => Promise.resolve(null)),
    checkRunFile: vi.fn(() => new Promise(() => {})),
    startRunFile: vi.fn(() => new Promise(() => {})),
    ...dumpStub(),
    listProcesses: vi.fn(() => Promise.resolve([])),
    ...securityStub(),
    signalProcess: vi.fn(() => Promise.resolve()),
  };
}

function connectionTransferStub(): Record<string, unknown> {
  return {
    cancelCredential: vi.fn().mockResolvedValue(undefined),
    testCredential: vi.fn().mockResolvedValue(undefined),
    exportConnections: vi.fn().mockResolvedValue(null),
    previewConnections: vi.fn().mockResolvedValue(null),
    importConnections: vi.fn().mockResolvedValue([]),
    cancelConnectionImport: vi.fn().mockResolvedValue(undefined),
  };
}

function securityStub(): Record<string, unknown> {
  return {
    securityStatus: vi.fn(() =>
      Promise.resolve({ locked: false, available: true, notice: null }),
    ),
    onSecurityChanged: vi.fn(() => Promise.resolve(() => undefined)),
    securityActivity: vi.fn(() => Promise.resolve()),
    lockApp: vi.fn(() => Promise.resolve()),
    unlockApp: vi.fn(() => Promise.resolve()),
  };
}

/** Native location and document stubs used without filesystem access. */
function appParityStub(): Record<string, unknown> {
  return {
    DEFAULT_CUSTOM_ENV_COLOR: '#7c3aed',
    getAppInfo: vi.fn(() => new Promise(() => {})),
    getDataLocations: vi.fn().mockResolvedValue(null),
    chooseDataLocation: vi.fn().mockResolvedValue(null),
    scheduleDataLocation: vi.fn().mockResolvedValue(undefined),
    takeNativeFiles: vi.fn().mockResolvedValue([]),
    onNativeFilesReady: vi.fn().mockResolvedValue(() => undefined),
    revealDatabase: vi.fn().mockResolvedValue(undefined),
  };
}

function mcpStub(): Record<string, unknown> {
  return {
    onMcpProposals: vi.fn().mockResolvedValue(() => undefined),
    investigateStatement: vi.fn().mockResolvedValue({
      statementId: '1',
      query: 'SELECT ?',
      intervals: [],
      calls: null,
      totalMs: null,
      meanMs: null,
      maxSinceResetMs: 5,
      regressions: [],
      partial: true,
    }),
    monitorAdvice: vi.fn().mockResolvedValue([]),
    proposeMonitorSql: vi.fn().mockResolvedValue(undefined),
    validateMonitorIndex: vi.fn().mockResolvedValue({
      kind: 'estimate',
      estimate: { baselineCost: 100, hypotheticalCost: 10 },
    }),
    explainMonitorQuery: vi
      .fn()
      .mockResolvedValue({ kind: 'blocked', message: 'test-only' }),
    takeMcpProposals: vi.fn().mockResolvedValue([]),
    monitorIncidents: vi
      .fn()
      .mockResolvedValue({ rules: [], incidents: [], background: false }),
    setMonitorBackground: vi.fn().mockResolvedValue(undefined),
    onMonitorIncident: vi.fn().mockResolvedValue(() => undefined),
  };
}

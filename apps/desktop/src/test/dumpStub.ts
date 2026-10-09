// SPDX-License-Identifier: Apache-2.0
import { vi } from 'vitest';
import type { DumpContextDto, DumpToolDto } from '@openrows/ipc';

/** A synthetic `pg_dump` found on the search path. */
export function tool(change: Partial<DumpToolDto> = {}): DumpToolDto {
  return {
    tool: 'pgDump',
    name: 'pg_dump',
    path: '/opt/pg/bin/pg_dump',
    version: 'PostgreSQL 17.4',
    fromSettings: false,
    warning: null,
    error: null,
    others: [],
    ...change,
  };
}

/** A local PostgreSQL database with every tool found. */
export function dumpContextOf(
  change: Partial<DumpContextDto> = {},
): DumpContextDto {
  return {
    engine: 'postgres',
    connection: 'Local',
    database: 'shop',
    readOnly: false,
    production: false,
    tools: [
      tool(),
      tool({
        tool: 'pgRestore',
        name: 'pg_restore',
        path: '/opt/pg/bin/pg_restore',
      }),
      tool({ tool: 'psql', name: 'psql', path: '/opt/pg/bin/psql' }),
    ],
    ...change,
  };
}

/** Backup and restore commands; nothing is picked or started by default. */
export function dumpStub(): Record<string, unknown> {
  return {
    dumpTools: vi.fn(() => Promise.resolve([])),
    dumpContext: vi.fn(() => Promise.resolve(dumpContextOf())),
    previewBackup: vi.fn(() =>
      Promise.resolve({ commands: ['pg_dump shop'], warnings: [] }),
    ),
    pickBackupFile: vi.fn(() => Promise.resolve(null)),
    startBackup: vi.fn(() => new Promise(() => {})),
    cancelDump: vi.fn(() => Promise.resolve()),
    pickRestoreFile: vi.fn(() => Promise.resolve(null)),
    restoreContents: vi.fn(() => Promise.resolve([])),
    checkRestore: vi.fn(() => new Promise(() => {})),
    startRestore: vi.fn(() => new Promise(() => {})),
  };
}

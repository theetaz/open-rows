// SPDX-License-Identifier: Apache-2.0
import type {
  BackupFormatDto,
  BackupOptionsDto,
  DumpContextDto,
  DumpFileKindDto,
  DumpScopeRequest,
  DumpToolKindDto,
  RestoreOptionsDto,
} from '@openrows/ipc';
import type { DumpTarget } from './dumpStore';

/** The session and server a request acts on. */
export function scopeOf(target: DumpTarget): DumpScopeRequest {
  return {
    sessionId: target.sessionId,
    serverProduct: target.serverProduct,
    serverVersion: target.serverVersion,
  };
}

export const FORMATS: readonly { value: BackupFormatDto; label: string }[] = [
  { value: 'custom', label: 'Custom archive (.dump)' },
  { value: 'plain', label: 'Plain SQL (.sql)' },
  { value: 'directory', label: 'Directory (one file per table)' },
  { value: 'tar', label: 'Tar archive (.tar)' },
];

/** The tools' own defaults: everything, a consistent MySQL snapshot. */
export const defaultBackupOptions: BackupOptionsDto = {
  format: 'custom',
  content: 'all',
  schemas: [],
  tables: [],
  noOwner: false,
  noPrivileges: false,
  compression: null,
  singleTransaction: true,
  routines: true,
  triggers: true,
  events: false,
};

export const defaultRestoreOptions: RestoreOptionsDto = {
  clean: 'keep',
  noOwner: false,
  noPrivileges: false,
  jobs: 1,
  stopOnError: true,
  selected: null,
};

/** The tool a backup runs on an engine. */
export function backupTool(context: DumpContextDto): DumpToolKindDto | null {
  if (context.engine === 'postgres') return 'pgDump';
  if (context.engine === 'mysql') return 'mysqldump';
  return null;
}

/** The tools a restore of `kind` runs on an engine. */
export function restoreTools(
  context: DumpContextDto,
  kind: DumpFileKindDto,
): readonly DumpToolKindDto[] {
  if (context.engine === 'mysql') return ['mysql'];
  if (context.engine === 'sqlite') return [];
  return kind === 'plainSql' ? ['psql'] : ['pgRestore', 'psql'];
}

/** A `pg_dump` archive (custom, tar or directory) with a table of contents. */
export function isArchive(kind: DumpFileKindDto): boolean {
  return kind === 'pgCustom' || kind === 'pgTar' || kind === 'pgDirectory';
}

export const FILE_KINDS: Readonly<Record<DumpFileKindDto, string>> = {
  plainSql: 'SQL script',
  pgCustom: 'pg_dump custom archive',
  pgTar: 'pg_dump tar archive',
  pgDirectory: 'pg_dump directory',
  sqlite: 'SQLite database',
};

export const messageOf = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

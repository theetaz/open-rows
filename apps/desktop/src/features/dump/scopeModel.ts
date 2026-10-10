// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { listObjects, listSchemas } from '@openrows/ipc';
import type {
  BackupOptionsDto,
  DumpEngineDto,
  DumpTableDto,
} from '@openrows/ipc';
import type { DumpTarget } from './dumpStore';
import { messageOf } from './model';

/** A listed schema and its tables, read only when the backup sheet opens. */
export type ScopeSchema = { name: string; tables: DumpTableDto[] };
/** Whole-database or explicit object selection; empty explicit scope backs up nothing. */
export type BackupSelection = {
  all: boolean;
  schemas: string[];
  tables: DumpTableDto[];
};

/** Initial context-menu scope, or the complete database for File > Back Up. */
export function initialSelection(target: DumpTarget): BackupSelection {
  return {
    all: !target.backupScope,
    schemas: target.backupScope?.schemas ?? [],
    tables: target.backupScope?.tables ?? [],
  };
}

/** Compare qualified names without conflating tables in different schemas. */
export function sameTable(a: DumpTableDto, b: DumpTableDto): boolean {
  return a.schema === b.schema && a.name === b.name;
}

/** Mixed PG scopes expand schemas into tables because pg_dump -t overrides -n. */
export function scopeOptions(
  selection: BackupSelection,
  catalog: readonly ScopeSchema[],
): Pick<BackupOptionsDto, 'schemas' | 'tables'> {
  if (selection.all) return { schemas: [], tables: [] };
  const extra = selection.tables.filter(
    (table) => !selection.schemas.includes(table.schema),
  );
  if (!extra.length) return { schemas: selection.schemas, tables: [] };
  const tables = [
    ...catalog
      .filter((schema) => selection.schemas.includes(schema.name))
      .flatMap((schema) => schema.tables),
    ...extra,
  ];
  return { schemas: [], tables };
}

async function readScope(target: DumpTarget): Promise<ScopeSchema[]> {
  const schemas = await listSchemas(target.profileId, target.database, true);
  const result: ScopeSchema[] = [];
  // Metadata reads share one session; sequential reads keep memory and load bounded.
  for (const schema of schemas.data.filter((item) => !item.system)) {
    const objects = await listObjects(
      target.profileId,
      target.database,
      { schema: schema.name, kinds: ['table'], limit: null },
      true,
    );
    result.push({
      name: schema.name,
      tables: objects.data.map((table) => ({
        schema: schema.name,
        name: table.name,
      })),
    });
  }
  return result;
}

/** Load the backup catalog; SQLite always backs up its complete file. */
export function useBackupScope(
  target: DumpTarget,
  engine: DumpEngineDto | undefined,
): { catalog: ScopeSchema[]; failure: string | null; loading: boolean } {
  const [state, setState] = useState({
    catalog: [] as ScopeSchema[],
    failure: null as string | null,
    loading: true,
  });
  useEffect(() => {
    if (!engine || engine === 'sqlite') return;
    let live = true;
    readScope(target).then(
      (catalog) => {
        if (live) setState({ catalog, failure: null, loading: false });
      },
      (error: unknown) => {
        if (live)
          setState({ catalog: [], failure: messageOf(error), loading: false });
      },
    );
    return () => {
      live = false;
    };
  }, [target, engine]);
  return state;
}

// SPDX-License-Identifier: Apache-2.0
import type { ColumnDto, EditInfoDto, EngineDto } from '@openrows/ipc';
import { selectSource } from './selectSource';

/** Conservative SQL provenance, before catalog key and connection checks. */
type ResultMapping =
  | { kind: 'table'; schema: string; table: string; columns: readonly string[] }
  | { kind: 'readonly'; reason: string };

/** Only direct columns of one explicitly qualified base relation can map to edits. */
export function resultMapping(
  sql: string,
  columns: readonly ColumnDto[],
  engine: EngineDto,
): ResultMapping {
  const source = selectSource(sql, engine);
  if (typeof source === 'string') return { kind: 'readonly', reason: source };
  const names = columns.map((column) => column.name);
  if (new Set(names).size !== names.length)
    return {
      kind: 'readonly',
      reason: 'Repeated result column names are ambiguous.',
    };
  if (source.columns !== null && source.columns.join('\0') !== names.join('\0'))
    return {
      kind: 'readonly',
      reason: 'Result columns do not match the direct SELECT columns.',
    };
  return {
    kind: 'table',
    schema: source.schema,
    table: source.table,
    columns: names,
  };
}

/** Catalog metadata remains authoritative for writable columns and usable keys. */
export function mappingReason(
  mapping: ResultMapping,
  info: EditInfoDto | null,
): string | null {
  if (mapping.kind === 'readonly') return mapping.reason;
  if (!info) return 'Checking the source table and its unique key…';
  if (info.readOnly) return 'The connection or source relation is read-only.';
  if (!info.key.length || info.rowId)
    return 'The source table has no supported primary or non-null unique key.';
  if (!info.key.every((key) => mapping.columns.includes(key)))
    return 'Include every primary or non-null unique key column in the SELECT.';
  if (
    !mapping.columns.every((name) =>
      info.columns.some((column) => column.name === name),
    )
  )
    return 'A result column is not a column of the source table.';
  return null;
}

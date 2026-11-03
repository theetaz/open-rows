// SPDX-License-Identifier: Apache-2.0
import type { ColumnDto, EditInfoDto, EngineDto } from '@openrows/ipc';
import { selectSource } from './selectSource';
import { t } from './lazyMessages';

/** Conservative SQL provenance, before catalog key and connection checks. */
export type ResultMapping =
  | { kind: 'table'; schema: string; table: string; columns: readonly string[] }
  | { kind: 'unresolved'; table: string; columns: readonly string[] }
  | { kind: 'readonly'; reason: string };

/** Only direct columns of one base relation can map to edits after resolution. */
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
      reason: t('repeatedResultColumn'),
    };
  if (source.columns !== null && source.columns.join('\0') !== names.join('\0'))
    return {
      kind: 'readonly',
      reason: t('resultColumnsDo'),
    };
  if (source.schema === null)
    return { kind: 'unresolved', table: source.table, columns: names };
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
  if (mapping.kind === 'unresolved') return t('resolvingSourceTable');
  if (!info) return t('checkingSourceTable');
  if (info.readOnly) return t('connectionSourceRelation');
  if (!info.key.length || info.rowId) return t('sourceTableNo');
  if (!info.key.every((key) => mapping.columns.includes(key)))
    return t('includeEveryPrimary');
  if (
    !mapping.columns.every((name) =>
      info.columns.some((column) => column.name === name),
    )
  )
    return t('resultColumnNot');
  return null;
}

/** A later statement may change the namespace before an unqualified source is mapped. */
export function mappingForBatch(
  mapping: ResultMapping,
  statementCount: number,
): ResultMapping {
  return mapping.kind === 'unresolved' && statementCount !== 1
    ? {
        kind: 'readonly',
        reason: t('runSelectOwn'),
      }
    : mapping;
}

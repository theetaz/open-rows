// SPDX-License-Identifier: Apache-2.0
import type { FileEncodingDto, TableColumnDto } from '@openrows/ipc';
import type { CellValue } from './cellValue';
import { formatJson } from './jsonFormat';

const textTypes = new Set([
  'text',
  'character varying',
  'varchar',
  'character',
  'char',
  'bpchar',
  'citext',
  'name',
]);

/** `character varying(255)` → `character varying`; `public.citext` → `citext`. */
function baseType(dataType: string): string {
  return dataType
    .replace(/\(.*\)$/, '')
    .replace(/^.*\./, '')
    .trim()
    .toLowerCase();
}

/** Scalar text types, where '' is a valid value (EMPTY). */
function isTextLike(dataType: string): boolean {
  return textTypes.has(baseType(dataType));
}

function isJson(dataType: string): boolean {
  const base = baseType(dataType);
  return base === 'json' || base === 'jsonb';
}

/** How "Add a file…" reads a file for this type, or `null` when unsupported. */
export function fileEncoding(dataType: string): FileEncodingDto | null {
  const base = baseType(dataType);
  if (base === 'bytea') return 'hex';
  return isTextLike(dataType) || isJson(dataType) || base === 'xml'
    ? 'text'
    : null;
}

/** Which value-menu actions apply; PRETTY/MINIFY carry their reformatted text. */
export type FieldActions = {
  null: boolean;
  default: boolean;
  empty: boolean;
  pretty: string | null;
  minify: string | null;
  file: FileEncodingDto | null;
};

const none: FieldActions = {
  null: false,
  default: false,
  empty: false,
  pretty: null,
  minify: null,
  file: null,
};

/**
 * Enable menu actions from catalog facts. `current` is the exact value, or
 * `null` while the exact snapshot is loading; `locked` covers generated
 * columns and rows staged for deletion.
 */
export function fieldActions(
  column: TableColumnDto | undefined,
  locked: boolean,
  current: CellValue | null,
): FieldActions {
  if (!column || locked) return none;
  const json = isJson(column.dataType) && current?.kind === 'text';
  return {
    null: column.nullable,
    default: column.nullable || column.default !== null,
    empty: isTextLike(column.dataType),
    pretty: json ? formatJson(current.value, true) : null,
    minify: json ? formatJson(current.value, false) : null,
    file: fileEncoding(column.dataType),
  };
}

/** Why `value` cannot be staged for `column`, or `null` when it can. */
export function fieldValueError(
  column: TableColumnDto | undefined,
  locked: boolean,
  value: CellValue,
): string | null {
  if (!column || locked) return 'This field cannot be edited.';
  const actions = fieldActions(column, false, null);
  if (value.kind === 'null' && !actions.null)
    return `${column.name} does not accept NULL.`;
  if (value.kind === 'default' && !actions.default)
    return `${column.name} has no DEFAULT and does not accept NULL.`;
  return null;
}

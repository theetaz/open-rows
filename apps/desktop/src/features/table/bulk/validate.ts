// SPDX-License-Identifier: Apache-2.0
import type { TableColumnDto } from '@openrows/ipc';
import type { CellValue } from '../cellValue';
import { fieldValueError } from '../fieldActions';

const INTEGER = /^[+-]?\d+$/;
const NUMBER = /^[+-]?(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?$/;
const SPECIAL_NUMBER = /^[+-]?(nan|infinity|inf)$/i;
const BOOLEAN = /^(t|f|true|false|yes|no|y|n|on|off|1|0)$/i;
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

/** `character varying(255)` → `character varying`; `public.mood` → `mood`. */
function baseType(dataType: string): string {
  return dataType
    .replace(/\(.*\)$/, '')
    .replace(/^.*\./, '')
    .replace(/\[\]$/, '')
    .trim()
    .toLowerCase();
}

/**
 * Why `text` is not valid input for a column type, or `null`. Only types
 * with an unambiguous text form are checked here (integers, numbers,
 * booleans, uuid, json); everything else is left to the server's input
 * function at commit, which still rolls the whole change set back.
 */
export function typeError(dataType: string, text: string): string | null {
  const type = baseType(dataType);
  if (dataType.endsWith('[]')) return null;
  if (
    /^(smallint|integer|bigint|int2|int4|int8|(small|big)?serial)$/.test(type)
  )
    return INTEGER.test(text.trim()) ? null : 'is not an integer';
  if (/^(numeric|decimal|real|double precision|float[48]?|money)$/.test(type))
    return NUMBER.test(text.trim()) || SPECIAL_NUMBER.test(text.trim())
      ? null
      : 'is not a number';
  if (type === 'boolean' || type === 'bool')
    return BOOLEAN.test(text.trim()) ? null : 'is not a boolean';
  if (type === 'uuid') return UUID.test(text.trim()) ? null : 'is not a UUID';
  if (type === 'json' || type === 'jsonb') return jsonError(text);
  return null;
}

function jsonError(text: string): string | null {
  try {
    JSON.parse(text);
    return null;
  } catch {
    return 'is not valid JSON';
  }
}

/**
 * Why `value` cannot be staged in `column` (catalog rules, then the type
 * check for text), or `null` when it can.
 */
export function cellError(
  column: TableColumnDto | undefined,
  locked: boolean,
  value: CellValue,
): string | null {
  const rule = fieldValueError(column, locked, value);
  if (rule || !column || value.kind !== 'text') return rule;
  const type = typeError(column.dataType, value.value);
  return type
    ? `${column.name}: ${JSON.stringify(value.value)} ${type}.`
    : null;
}

/** One line per problem, capped so the message stays readable. */
export function summarize(
  problems: readonly { row: string; message: string }[],
  cap = 5,
): string {
  const lines = problems
    .slice(0, cap)
    .map((problem) => `${problem.row} · ${problem.message}`);
  const more = problems.length - lines.length;
  if (more > 0) lines.push(`… and ${String(more)} more`);
  return `Nothing was staged. ${lines.join('\n')}`;
}

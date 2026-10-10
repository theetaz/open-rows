// SPDX-License-Identifier: Apache-2.0
import { editorDialect } from '../connections/engines';
import type { EngineDto, FilterOperatorDto } from '@openrows/ipc';
import { columnAlign } from '../../results/align';
import { isBooleanType } from './inline/cellKind';
import { operators } from './model';
import type { OperatorInfo } from './model';

/** Ordering and equality for every type. */
const COMMON: readonly FilterOperatorDto[] = [
  'equal',
  'notEqual',
  'in',
  'notIn',
  'isNull',
  'isNotNull',
];
/** Ranges for numbers, dates and text. */
const ORDERED: readonly FilterOperatorDto[] = [
  'less',
  'greater',
  'lessOrEqual',
  'greaterOrEqual',
  'between',
  'notBetween',
];
/** Pattern and emptiness checks on the text form. */
const TEXT: readonly FilterOperatorDto[] = [
  'contains',
  'notContains',
  'startsWith',
  'endsWith',
  'like',
  'notLike',
  'regex',
  'notRegex',
  'isEmpty',
  'isNotEmpty',
];
const REGEX = new Set<FilterOperatorDto>(['regex', 'notRegex']);
const TEMPORAL = /date|time|interval|year/i;

/** What a column type can be compared with: boolean, ordered or text. */
function allowed(dataType: string | null): ReadonlySet<FilterOperatorDto> {
  if (dataType && isBooleanType(dataType)) return new Set(COMMON);
  if (dataType && (columnAlign(dataType) === 'end' || TEMPORAL.test(dataType)))
    return new Set([...COMMON, ...ORDERED]);
  return new Set([...COMMON, ...ORDERED, ...TEXT]);
}

/**
 * The operators a filter row offers for a column (FR-GRID-11): booleans
 * compare for equality, numbers and dates also by range, and text also by
 * pattern; regex only where the engine has it (not SQLite). An unknown
 * type offers everything the engine supports.
 */
export function operatorsFor(
  dataType: string | null,
  engine: EngineDto,
): OperatorInfo[] {
  const kinds = allowed(dataType);
  return operators.filter(
    (item) =>
      kinds.has(item.value) &&
      !(editorDialect(engine) === 'sqlite' && REGEX.has(item.value)),
  );
}

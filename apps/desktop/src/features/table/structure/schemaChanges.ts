// SPDX-License-Identifier: Apache-2.0
import type {
  DefaultDto,
  SchemaChangeDto,
  TableDetailDto,
} from '@openrows/ipc';
import type { History } from '../editHistory';

/** One staged structure change; keyed changes replace earlier ones. */
export type StagedSchema = { id: string; change: SchemaChangeDto };
export type SchemaHistory = History<StagedSchema>;

/** What a staged change is compared with to drop no-ops. */
type SchemaBase = { detail: TableDetailDto; table: string };

/** One staged change per property; added columns and indexes keep their own id. */
function changeKey(change: SchemaChangeDto): string | null {
  switch (change.kind) {
    case 'renameTable':
      return 'table:name';
    case 'tableComment':
      return 'table:comment';
    case 'dropIndex':
      return `dropIndex:${change.name}`;
    case 'addColumn':
    case 'createIndex':
      return null;
    default:
      return `${change.kind}:${change.column}`;
  }
}

/** Empty comments mean "no comment". */
const comment = (text: string | null): string | null =>
  text === '' ? null : text;

/** Whether `value` is what the catalog already has. */
function sameDefault(value: DefaultDto, original: string | null): boolean {
  if (value.kind === 'none') return original === null;
  return value.kind === 'expression' && value.sql.trim() === original;
}

/** Whether the change leaves the catalog as it is. */
function isNoop(change: SchemaChangeDto, base: SchemaBase): boolean {
  if (change.kind === 'renameTable') return change.name === base.table;
  if (change.kind === 'tableComment')
    return comment(change.comment) === base.detail.info.comment;
  if (!('column' in change) || change.kind === 'dropColumn') return false;
  const column = base.detail.columns.find(
    (item) => item.name === change.column,
  );
  if (!column) return false;
  switch (change.kind) {
    case 'renameColumn':
      return change.name === column.name;
    case 'columnType':
      return change.dataType.trim() === column.dataType;
    case 'columnNullable':
      return change.nullable === column.nullable;
    case 'columnDefault':
      return sameDefault(change.default, column.default);
    case 'columnComment':
      return comment(change.comment) === column.comment;
  }
}

function push(history: SchemaHistory, current: StagedSchema[]): SchemaHistory {
  return {
    pending: false,
    current,
    past: [...history.past.slice(-49), history.current],
    future: [],
  };
}

const locked = (history: SchemaHistory): boolean =>
  history.pending || Boolean(history.uncertain);

/**
 * Stage `change`: it replaces the earlier change of the same property, is
 * dropped when it restores the catalog value, and dropping a column removes
 * that column's other staged changes.
 */
export function stageSchema(
  history: SchemaHistory,
  change: SchemaChangeDto,
  base: SchemaBase,
): SchemaHistory {
  if (locked(history)) return history;
  const key = changeKey(change);
  const current = history.current.filter((item) => {
    if (key !== null && item.id === key) return false;
    return !(
      change.kind === 'dropColumn' &&
      'column' in item.change &&
      item.change.column === change.column
    );
  });
  if (!isNoop(change, base))
    current.push({ id: key ?? crypto.randomUUID(), change });
  return push(history, current);
}

/** Replace a staged added column or index (edited before commit). */
export function replaceStaged(
  history: SchemaHistory,
  id: string,
  change: SchemaChangeDto,
): SchemaHistory {
  if (locked(history)) return history;
  return push(
    history,
    history.current.map((item) => (item.id === id ? { id, change } : item)),
  );
}

/** Remove one staged change (an added column, a drop, …). */
export function unstage(history: SchemaHistory, id: string): SchemaHistory {
  if (locked(history)) return history;
  return push(
    history,
    history.current.filter((item) => item.id !== id),
  );
}

/** `'it''s'`: a literal default as SQL text, for display only. */
export function defaultText(value: DefaultDto): string | null {
  if (value.kind === 'none') return null;
  if (value.kind === 'expression') return value.sql;
  return `'${value.value.replaceAll("'", "''")}'`;
}

/** One line per staged change for the pending-changes list. */
export function describe(change: SchemaChangeDto): string {
  switch (change.kind) {
    case 'renameTable':
      return `Rename table to ${change.name}`;
    case 'tableComment':
      return 'Change table comment';
    case 'renameColumn':
      return `Rename column ${change.column} to ${change.name}`;
    case 'columnType':
      return `Change type of ${change.column} to ${change.dataType}`;
    case 'columnNullable':
      return `${change.nullable ? 'Allow' : 'Disallow'} NULL in ${change.column}`;
    case 'columnDefault':
      return `Change default of ${change.column}`;
    case 'columnComment':
      return `Change comment of ${change.column}`;
    case 'addColumn':
      return `Add column ${change.name} ${change.dataType}`;
    case 'dropColumn':
      return `Delete column ${change.column}`;
    case 'createIndex':
      return `Create index ${change.name ?? ''} on ${change.columns.join(', ')}`;
    case 'dropIndex':
      return `Delete index ${change.name}`;
  }
}

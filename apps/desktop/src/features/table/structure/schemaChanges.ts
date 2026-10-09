// SPDX-License-Identifier: Apache-2.0
import type {
  DefaultDto,
  GeneratedDto,
  IdentityDto,
  SchemaChangeDto,
  TableDetailDto,
} from '@openrows/ipc';
import type { History } from '../editHistory';

/** One staged structure change; keyed changes replace earlier ones. */
export type StagedSchema = { id: string; change: SchemaChangeDto };
export type SchemaHistory = History<StagedSchema>;

/** What a staged change is compared with to drop no-ops. */
export type SchemaBase = { detail: TableDetailDto; table: string };

/** One staged change per property; added columns and indexes keep their own id. */
function changeKey(change: SchemaChangeDto): string | null {
  switch (change.kind) {
    case 'renameTable':
      return 'table:name';
    case 'tableComment':
      return 'table:comment';
    case 'dropIndex':
      return `dropIndex:${change.name}`;
    case 'dropConstraint':
      return `dropConstraint:${change.name}`;
    case 'addColumn':
    case 'createIndex':
    case 'addForeignKey':
    case 'addCheck':
      return null;
    default:
      return `${change.kind}:${change.column}`;
  }
}

/** The identity a column has now, as the change would set it. */
function identityOf(generated: GeneratedDto): IdentityDto {
  switch (generated) {
    case 'identityAlways':
      return 'always';
    case 'identityByDefault':
      return 'byDefault';
    case 'autoIncrement':
      return 'autoIncrement';
    default:
      return 'none';
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
    case 'columnIdentity':
      return change.identity === identityOf(column.generated);
    case 'columnCollation':
      return false;
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

/** How a numbering choice reads in lists and pickers. */
const identityLabel: Record<IdentityDto, string> = {
  none: 'none',
  always: 'GENERATED ALWAYS AS IDENTITY',
  byDefault: 'GENERATED BY DEFAULT AS IDENTITY',
  autoIncrement: 'AUTO_INCREMENT',
};

type ColumnChange = Extract<SchemaChangeDto, { column: string }>;

/** One line for a change of an existing column. */
function describeColumn(change: ColumnChange): string {
  switch (change.kind) {
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
    case 'columnIdentity':
      return `Change numbering of ${change.column} to ${identityLabel[change.identity]}`;
    case 'columnCollation':
      return `Change collation of ${change.column}`;
    case 'dropColumn':
      return `Delete column ${change.column}`;
  }
}

/** One line per staged change for the pending-changes list. */
export function describe(change: SchemaChangeDto): string {
  if ('column' in change) return describeColumn(change);
  switch (change.kind) {
    case 'renameTable':
      return `Rename table to ${change.name}`;
    case 'tableComment':
      return 'Change table comment';
    case 'addColumn':
      return `Add column ${change.name} ${change.dataType}`;
    case 'createIndex':
      return `Create index ${change.name ?? ''} on ${change.columns.join(', ')}`;
    case 'dropIndex':
      return `Delete index ${change.name}`;
    case 'addForeignKey':
      return `Add foreign key ${change.key.name ?? ''} → ${change.key.target.table}`;
    case 'addCheck':
      return `Add check ${change.check.name ?? change.check.expression}`;
    case 'dropConstraint':
      return `Delete constraint ${change.name}`;
  }
}

// SPDX-License-Identifier: Apache-2.0
import type {
  DefaultDto,
  ForeignKeyDto,
  GeneratedDto,
  IdentityDto,
  SchemaChangeDto,
  StructureColumnDto,
  TableDetailDto,
} from '@openrows/ipc';
import { defaultText } from './schemaChanges';
import type { StagedSchema } from './schemaChanges';

/** Editable fields of a Structure row. */
export type ColumnField =
  'name' | 'dataType' | 'nullable' | 'default' | 'comment';

/** A Structure grid row: the catalog column with staged changes applied. */
export type ColumnRow = {
  /** Catalog name, or the staged id of an added column. */
  id: string;
  /** Catalog name; `null` for a staged new column. */
  column: string | null;
  name: string;
  dataType: string;
  nullable: boolean;
  /** Default as SQL text (a literal shows quoted). */
  default: string | null;
  generated: GeneratedDto;
  comment: string | null;
  check: string | null;
  foreignKey: ForeignKeyDto | null;
  dropped: boolean;
  changed: readonly ColumnField[];
};

/** An Index grid row: catalog index, staged deletion or staged new index. */
export type IndexRow = {
  id: string;
  name: string;
  method: string;
  unique: boolean;
  primary: boolean;
  constraint: boolean;
  columns: string;
  include: string;
  predicate: string | null;
  comment: string | null;
  dropped: boolean;
  added: boolean;
};

type ColumnChange = Extract<SchemaChangeDto, { column: string }>;

function applyChange(row: ColumnRow, change: ColumnChange): ColumnRow {
  const mark = (field: ColumnField): readonly ColumnField[] => [
    ...row.changed,
    field,
  ];
  switch (change.kind) {
    case 'renameColumn':
      return { ...row, name: change.name, changed: mark('name') };
    case 'columnType':
      return { ...row, dataType: change.dataType, changed: mark('dataType') };
    case 'columnNullable':
      return { ...row, nullable: change.nullable, changed: mark('nullable') };
    case 'columnDefault':
      return {
        ...row,
        default: defaultText(change.default),
        changed: mark('default'),
      };
    case 'columnComment':
      return { ...row, comment: change.comment, changed: mark('comment') };
    case 'columnIdentity':
      return { ...row, generated: generatedOf(change.identity) };
    case 'columnCollation':
      return row;
    case 'dropColumn':
      return { ...row, dropped: true };
  }
}

/** The generation kind a staged numbering choice shows as. */
function generatedOf(identity: IdentityDto): GeneratedDto {
  switch (identity) {
    case 'always':
      return 'identityAlways';
    case 'byDefault':
      return 'identityByDefault';
    case 'autoIncrement':
      return 'autoIncrement';
    case 'none':
      return 'no';
  }
}

function catalogRow(column: StructureColumnDto): ColumnRow {
  return {
    id: column.name,
    column: column.name,
    name: column.name,
    dataType: column.dataType,
    nullable: column.nullable,
    default: column.default,
    generated: column.generated,
    comment: column.comment,
    check: column.check,
    foreignKey: column.foreignKey,
    dropped: false,
    changed: [],
  };
}

/** A staged `addColumn` as a row. */
function addedRow(id: string, change: AddColumn): ColumnRow {
  return {
    id,
    column: null,
    name: change.name,
    dataType: change.dataType,
    nullable: change.nullable,
    default: defaultText(change.default),
    generated: generatedOf(change.identity ?? 'none'),
    comment: change.comment,
    check: null,
    foreignKey: null,
    dropped: false,
    changed: ['name', 'dataType', 'nullable', 'default', 'comment'],
  };
}

export type AddColumn = Extract<SchemaChangeDto, { kind: 'addColumn' }>;

/** Catalog columns with staged changes, then staged new columns. */
export function columnRows(
  detail: TableDetailDto,
  staged: readonly StagedSchema[],
): ColumnRow[] {
  const rows = detail.columns.map((column) =>
    staged.reduce(
      (row, { change }) =>
        'column' in change && change.column === row.column
          ? applyChange(row, change)
          : row,
      catalogRow(column),
    ),
  );
  for (const { id, change } of staged)
    if (change.kind === 'addColumn') rows.push(addedRow(id, change));
  return rows;
}

/** Catalog indexes (staged deletions marked), then staged new indexes. */
export function indexRows(
  detail: TableDetailDto,
  staged: readonly StagedSchema[],
): IndexRow[] {
  const dropped = new Set(
    staged.flatMap(({ change }) =>
      change.kind === 'dropIndex' ? [change.name] : [],
    ),
  );
  const rows: IndexRow[] = detail.indexes.map((index) => ({
    ...index,
    id: index.name,
    include: index.include.join(', '),
    dropped: dropped.has(index.name),
    added: false,
  }));
  for (const { id, change } of staged)
    if (change.kind === 'createIndex')
      rows.push({
        id,
        name: change.name ?? '',
        method: change.method,
        unique: change.unique,
        primary: false,
        constraint: false,
        columns: change.columns.join(', '),
        include: '',
        predicate: null,
        comment: null,
        dropped: false,
        added: true,
      });
  return rows;
}

/** The staged value of a table-level property, else the catalog's. */
export function tableValue<K extends 'renameTable' | 'tableComment'>(
  staged: readonly StagedSchema[],
  kind: K,
): Extract<SchemaChangeDto, { kind: K }> | undefined {
  const found = staged.find((item) => item.change.kind === kind);
  return found?.change as Extract<SchemaChangeDto, { kind: K }> | undefined;
}

/** A default typed into the grid: empty means none, else an expression. */
export function typedDefault(text: string, literal: boolean): DefaultDto {
  if (literal) return { kind: 'literal', value: text };
  return text.trim()
    ? { kind: 'expression', sql: text.trim() }
    : { kind: 'none' };
}

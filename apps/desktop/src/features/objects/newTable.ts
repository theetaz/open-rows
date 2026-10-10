// SPDX-License-Identifier: Apache-2.0
import { editorDialect } from '../connections/engines';
import type {
  EngineDto,
  NewCheckDto,
  ObjectChangeDto,
  ObjectFeaturesDto,
} from '@openrows/ipc';
import { cleanCheck } from '../table/structure/forms/CheckFields';
import {
  columnProblem,
  emptyColumn,
  newColumn,
} from '../table/structure/forms/columnDraft';
import type { ColumnDraft } from '../table/structure/forms/columnDraft';
import { cleanForeignKey } from '../table/structure/forms/ForeignKeyFields';
import type { ForeignKeyDraft } from '../table/structure/forms/ForeignKeyFields';
import { cleanIndex } from '../table/structure/forms/IndexFields';
import type { IndexDraft } from '../table/structure/forms/IndexFields';

/** What the new-table editor edits. */
export type TableDraft = {
  name: string;
  columns: ColumnDraft[];
  primaryKey: string[];
  indexes: IndexDraft[];
  foreignKeys: ForeignKeyDraft[];
  checks: NewCheckDto[];
  comment: string;
};

/** A table with one `id` key column, numbered where the engine can. */
export function emptyTable(
  engine: EngineDto,
  features: ObjectFeaturesDto,
): TableDraft {
  const mysql = engine === 'mysql' || engine === 'mariadb';
  const id: ColumnDraft = {
    ...emptyColumn('id', engine),
    type:
      editorDialect(engine) === 'sqlite'
        ? 'INTEGER'
        : mysql
          ? 'bigint unsigned'
          : 'bigint',
    nullable: false,
    identity: !features.identity ? 'none' : mysql ? 'autoIncrement' : 'always',
  };
  return {
    name: '',
    columns: [id],
    primaryKey: ['id'],
    indexes: [],
    foreignKeys: [],
    checks: [],
    comment: '',
  };
}

/** Why the table cannot be created yet, or `null`. */
export function tableProblem(draft: TableDraft): string | null {
  if (!draft.name.trim()) return 'Name the table.';
  if (!draft.columns.length) return 'Add at least one column.';
  for (const [index, column] of draft.columns.entries()) {
    const problem = columnProblem(column);
    if (problem) return `Column ${String(index + 1)}: ${problem}`;
  }
  const names = draft.columns.map((column) => column.name.trim());
  const twice = names.find((name, index) => names.indexOf(name) !== index);
  return twice ? `Two columns are named ${twice}.` : null;
}

/** Rename a column everywhere the draft names it. */
export function renameColumn(
  draft: TableDraft,
  from: string,
  to: string,
): TableDraft {
  const swap = (name: string): string => (name === from ? to : name);
  return {
    ...draft,
    primaryKey: draft.primaryKey.map(swap),
    indexes: draft.indexes.map((index) => ({
      ...index,
      columns: index.columns.map(swap),
      descending: index.descending.map(swap),
    })),
    foreignKeys: draft.foreignKeys.map((key) => ({
      ...key,
      columns: key.columns.map(swap),
    })),
  };
}

/** The `createTable` request of the draft in `schema`. */
export function createTableChange(
  draft: TableDraft,
  schema: string,
): ObjectChangeDto {
  const known = new Set(draft.columns.map((column) => column.name.trim()));
  return {
    kind: 'createTable',
    target: { schema, name: draft.name.trim() },
    definition: {
      columns: draft.columns.map(newColumn),
      primaryKey: draft.primaryKey.filter((name) => known.has(name)),
      indexes: draft.indexes.map(cleanIndex),
      foreignKeys: draft.foreignKeys.map(cleanForeignKey),
      checks: draft.checks.map(cleanCheck),
      comment: draft.comment.trim() || null,
    },
  };
}

// SPDX-License-Identifier: Apache-2.0
import { newColumn } from './columnEdits';
import type { InlineEdit } from './columnCells';
import type { ColumnRow, IndexRow } from './effective';
import type { MenuItem, MenuState } from './ItemMenu';
import type { Cursor } from './StructureGrid';
import type { SchemaEditing } from './useSchemaEditing';
import { t } from '../messages/lazyStructure';

type Point = { x: number; y: number };

/** Commands of the Structure and Index views (menus, keys, footer). */
export type StructureActions = {
  addColumn: () => void;
  dropRow: (row: ColumnRow) => void;
  dropIndex: (row: IndexRow) => void;
  openTable: (row: ColumnRow) => void;
  rowMenu: (row: ColumnRow, at: Point) => MenuState;
  indexMenu: (row: IndexRow, at: Point) => MenuState;
};

type Context = {
  schema: SchemaEditing;
  rows: readonly ColumnRow[];
  setCursor: (cursor: Cursor) => void;
  setEdit: (edit: InlineEdit | null) => void;
  setSearch: (search: string) => void;
  onOpenTable?: ((schema: string, table: string) => void) | undefined;
  openColumnSheet: (row: ColumnRow | null) => void;
};

const copy = (text: string): void => {
  navigator.clipboard.writeText(text).catch(() => undefined);
};

export function structureActions(context: Context): StructureActions {
  const { schema } = context;
  const dropRow = (row: ColumnRow): void => {
    if (row.column === null) schema.unstage(row.id);
    else schema.stage({ kind: 'dropColumn', column: row.column });
  };
  const dropIndex = (row: IndexRow): void => {
    if (row.added) schema.unstage(row.id);
    else if (!row.constraint)
      schema.stage({ kind: 'dropIndex', name: row.name });
  };
  const openTable = (row: ColumnRow): void => {
    if (row.foreignKey)
      context.onOpenTable?.(row.foreignKey.schema, row.foreignKey.table);
  };
  return {
    addColumn: () => {
      if (!schema.enabled) return;
      const change = newColumn(context.rows);
      schema.stage(change);
      context.setSearch('');
      const cursor = { row: context.rows.length, col: 1 };
      context.setCursor(cursor);
      context.setEdit({ ...cursor, text: change.name, literal: false });
    },
    dropRow,
    dropIndex,
    openTable,
    rowMenu: (row, at) => ({
      ...at,
      label: t('columnName2', { name: row.name }),
      items: rowItems(row, {
        schema,
        dropRow,
        openTable,
        edit: context.openColumnSheet,
      }),
    }),
    indexMenu: (row, at) => ({
      ...at,
      label: t('indexName2', { name: row.name }),
      items: indexItems(row, schema, dropIndex),
    }),
  };
}

function rowItems(
  row: ColumnRow,
  act: {
    schema: SchemaEditing;
    dropRow: (row: ColumnRow) => void;
    openTable: (row: ColumnRow) => void;
    edit: (row: ColumnRow | null) => void;
  },
): MenuItem[] {
  const items: MenuItem[] = [];
  if (!row.dropped)
    items.push({
      label: t('editColumn'),
      disabled: !act.schema.enabled,
      run: () => {
        act.edit(row);
      },
    });
  if (row.dropped && row.column !== null) {
    const column = row.column;
    items.push({
      label: t('restoreColumn'),
      disabled: !act.schema.enabled,
      run: () => {
        act.schema.unstage(`dropColumn:${column}`);
      },
    });
  } else
    items.push({
      label: row.column === null ? t('removeNewColumn') : t('deleteColumn'),
      danger: true,
      disabled: !act.schema.enabled,
      run: () => {
        act.dropRow(row);
      },
    });
  items.push(
    {
      label: t('addColumn2'),
      disabled: !act.schema.enabled,
      run: () => {
        act.edit(null);
      },
    },
    {
      label: t('copyName'),
      run: () => {
        copy(row.name);
      },
    },
  );
  if (row.foreignKey)
    items.push({
      label: t('openTable', { table: row.foreignKey.table }),
      run: () => {
        act.openTable(row);
      },
    });
  return items;
}

function indexItems(
  row: IndexRow,
  schema: SchemaEditing,
  dropIndex: (row: IndexRow) => void,
): MenuItem[] {
  const drop: MenuItem = row.dropped
    ? {
        label: t('restoreIndex'),
        disabled: !schema.enabled,
        run: () => {
          schema.unstage(`dropIndex:${row.name}`);
        },
      }
    : {
        label: row.added ? t('removeNewIndex') : t('deleteIndex'),
        danger: true,
        disabled: !schema.enabled || row.constraint,
        ...(row.constraint
          ? {
              title: t('indexBacksConstraint'),
            }
          : {}),
        run: () => {
          dropIndex(row);
        },
      };
  return [
    drop,
    {
      label: t('copyName'),
      disabled: !row.name,
      run: () => {
        copy(row.name);
      },
    },
  ];
}

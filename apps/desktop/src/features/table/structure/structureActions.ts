// SPDX-License-Identifier: Apache-2.0
import { newColumn } from './columnEdits';
import type { InlineEdit } from './columnCells';
import type { ColumnRow, IndexRow } from './effective';
import type { MenuItem, MenuState } from './ItemMenu';
import type { Cursor } from './StructureGrid';
import type { SchemaEditing } from './useSchemaEditing';

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
      label: `Column ${row.name}`,
      items: rowItems(row, { schema, dropRow, openTable }),
    }),
    indexMenu: (row, at) => ({
      ...at,
      label: `Index ${row.name}`,
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
  },
): MenuItem[] {
  const items: MenuItem[] = [];
  if (row.dropped && row.column !== null) {
    const column = row.column;
    items.push({
      label: 'Restore column',
      disabled: !act.schema.enabled,
      run: () => {
        act.schema.unstage(`dropColumn:${column}`);
      },
    });
  } else
    items.push({
      label: row.column === null ? 'Remove new column' : 'Delete column',
      danger: true,
      disabled: !act.schema.enabled,
      run: () => {
        act.dropRow(row);
      },
    });
  items.push({
    label: 'Copy name',
    run: () => {
      copy(row.name);
    },
  });
  if (row.foreignKey)
    items.push({
      label: `Open ${row.foreignKey.table}`,
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
        label: 'Restore index',
        disabled: !schema.enabled,
        run: () => {
          schema.unstage(`dropIndex:${row.name}`);
        },
      }
    : {
        label: row.added ? 'Remove new index' : 'Delete index',
        danger: true,
        disabled: !schema.enabled || row.constraint,
        ...(row.constraint
          ? {
              title:
                'This index backs a constraint; drop the constraint instead',
            }
          : {}),
        run: () => {
          dropIndex(row);
        },
      };
  return [
    drop,
    {
      label: 'Copy name',
      disabled: !row.name,
      run: () => {
        copy(row.name);
      },
    },
  ];
}

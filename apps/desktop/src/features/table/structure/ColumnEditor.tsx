// SPDX-License-Identifier: Apache-2.0
import { useRef } from 'react';
import type { ColumnRow } from './effective';
import { CellInput, columnCell } from './columnCells';
import { StructureGrid } from './StructureGrid';
import type { GridColumnDef } from './StructureGrid';
import { useColumnEditor } from './useColumnEditor';
import type { Structure } from './useStructure';

const gridColumns: readonly GridColumnDef[] = [
  { key: 'number', title: '#', width: 44 },
  { key: 'name', title: 'column_name', width: 180 },
  { key: 'dataType', title: 'data_type', width: 170 },
  { key: 'nullable', title: 'is_nullable', width: 100 },
  { key: 'check', title: 'check', width: 150 },
  { key: 'default', title: 'column_default', width: 230 },
  { key: 'foreignKey', title: 'foreign_key', width: 170 },
  { key: 'comment', title: 'comment', width: 240 },
];

/** Structure mode: the editable column grid (TablePlus Structure tab). */
export function ColumnEditor({
  structure,
}: {
  structure: Structure;
}): React.JSX.Element {
  const grid = useRef<HTMLDivElement>(null);
  const editor = useColumnEditor(structure, grid);
  const { schema, columnGrid: state } = structure;
  const { shown, at } = editor;
  return (
    <StructureGrid
      ref={grid}
      label="Columns"
      columns={gridColumns}
      rows={shown.length}
      rowKey={(row) => at(row)?.id ?? String(row)}
      rowState={(row) => rowState(at(row))}
      cell={(row, col) => {
        const item = shown[row];
        if (!item) return { content: null };
        if (state.edit?.row === row && state.edit.col === col)
          return {
            content: (
              <CellInput
                edit={state.edit}
                label={`Edit ${gridColumns[col]?.title ?? ''} of ${item.row.name}`}
                onText={(text) => {
                  if (state.edit) state.setEdit({ ...state.edit, text });
                }}
                onDone={editor.finish}
              />
            ),
          };
        return columnCell(item.row, [item.index, col], {
          editable: schema.enabled,
          open: (name, _index, point) => {
            editor.openPicker(name, row, point);
          },
          openTable: structure.openTable,
        });
      }}
      cursor={state.cursor}
      onCursor={state.setCursor}
      onKey={editor.keyDown}
      onEdit={(cursor) => {
        editor.startEdit(cursor);
      }}
      onMenu={(row, point) => {
        const item = at(row);
        if (item) structure.setMenu(structure.rowMenu(item, point));
      }}
      onFiller={schema.enabled ? structure.addColumn : undefined}
      fillerLabel={schema.enabled ? 'Click to add a column' : ''}
    />
  );
}

function rowState(row: ColumnRow | undefined): 'dropped' | 'added' | undefined {
  if (row?.dropped) return 'dropped';
  return row?.column === null ? 'added' : undefined;
}

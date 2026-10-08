// SPDX-License-Identifier: Apache-2.0
import type { IndexRow } from './effective';
import { StructureGrid } from './StructureGrid';
import type { CellView, GridColumnDef } from './StructureGrid';
import type { Structure } from './useStructure';

const gridColumns: readonly GridColumnDef[] = [
  { key: 'number', title: '#', width: 44 },
  { key: 'name', title: 'index_name', width: 220 },
  { key: 'method', title: 'index_algorithm', width: 130 },
  { key: 'unique', title: 'is_unique', width: 90 },
  { key: 'primary', title: 'is_primary', width: 90 },
  { key: 'columns', title: 'column_names', width: 220 },
  { key: 'predicate', title: 'condition', width: 200 },
  { key: 'include', title: 'include', width: 150 },
  { key: 'comment', title: 'comment', width: 200 },
];

const yes = (flag: boolean): string => (flag ? 'TRUE' : 'FALSE');
const text = (value: string | null, empty: 'NULL' | 'EMPTY'): CellView =>
  value ? { content: value } : { content: empty, muted: true };

function indexCell(row: IndexRow, index: number, col: number): CellView {
  switch (col) {
    case 0:
      return { content: index + 1 };
    case 1:
      return row.name
        ? { content: row.name }
        : { content: '(automatic name)', muted: true };
    case 2:
      return { content: row.method };
    case 3:
      return { content: yes(row.unique) };
    case 4:
      return { content: yes(row.primary) };
    case 5:
      return { content: row.columns };
    case 6:
      return text(row.predicate, 'NULL');
    case 7:
      return text(row.include, 'EMPTY');
    default:
      return text(row.comment, 'NULL');
  }
}

/** Index mode: the table's indexes, staged deletions and new indexes. */
export function IndexEditor({
  structure,
}: {
  structure: Structure;
}): React.JSX.Element {
  const { indexes, indexGrid } = structure;
  return (
    <StructureGrid
      label="Indexes"
      columns={gridColumns}
      rows={indexes.length}
      rowKey={(row) => indexes[row]?.id ?? String(row)}
      rowState={(row) => {
        const item = indexes[row];
        if (item?.dropped) return 'dropped';
        return item?.added ? 'added' : undefined;
      }}
      cell={(row, col) => {
        const item = indexes[row];
        return item ? indexCell(item, row, col) : { content: null };
      }}
      cursor={indexGrid.cursor}
      onCursor={indexGrid.setCursor}
      onKey={(event, cursor) => {
        const item = indexes[cursor.row];
        if (!item || !structure.schema.enabled) return false;
        if (event.key !== 'Delete' && event.key !== 'Backspace') return false;
        event.preventDefault();
        structure.dropIndex(item);
        return true;
      }}
      onEdit={() => undefined}
      onMenu={(row, point) => {
        const item = indexes[row];
        if (item) structure.setMenu(structure.indexMenu(item, point));
      }}
      onFiller={structure.schema.enabled ? structure.openIndexSheet : undefined}
      fillerLabel={structure.schema.enabled ? 'Click to add an index' : ''}
    />
  );
}

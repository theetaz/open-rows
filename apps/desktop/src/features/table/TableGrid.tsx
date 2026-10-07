// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef, useState } from 'react';
import { TileSource } from '@openrows/grid';
import type {
  GridColumn,
  GridProps,
  GridContextTarget,
  GridDataSource,
} from '@openrows/grid';
import type { ColumnDto, SortDto } from '@openrows/ipc';
import { mountCanvas } from '../../results/mountCanvas';
import { useGridAppearance } from '../../results/useGridAppearance';
import { displayWindow } from '../../results/displayWindow';
import type { ResultMeta } from '../query/run';
import { EditedSource } from './EditedSource';
import type { StagedEdit } from './editHistory';
import { NumberedSource } from './NumberedSource';
import styles from './Table.module.css';
import { columnAlign } from '../../results/align';

/** Header title with the server-side sort direction, e.g. `name ▲`. */
function headerTitle(name: string, sort: SortDto | null): string {
  if (sort?.column !== name) return name;
  return `${name} ${sort.descending ? '▼' : '▲'}`;
}

function columnsOf(
  columns: readonly ColumnDto[],
  sort: SortDto | null,
): GridColumn[] {
  return columns.map((column, index) => ({
    id: String(index),
    title: headerTitle(column.name, sort),
    width: Math.min(
      320,
      Math.max(columnWidth(column.sourceType), column.name.length * 8 + 56),
    ),
    pinned: false,
    align: columnAlign(column.sourceType),
  }));
}

type Props = {
  result: ResultMeta;
  sort: SortDto | null;
  /** Header click on a column (sorts server-side). */
  onHeader: (column: number) => void;
  /** Row focus changed. */
  onRow: (row: number) => void;
  changes: readonly StagedEdit[];
  editKeys: readonly string[];
  offset: number;
  rowNumbers: boolean;
  striped: boolean;
  onEdit?: (cell: { row: number; column: number }) => void;
  onContextMenu: (target: GridContextTarget) => void;
};

/** Canvas grid over one page's result tiles; rows never enter React state. */
export function TableGrid({
  result,
  sort,
  onHeader,
  onRow,
  offset,
  rowNumbers,
  striped,
  onEdit,
  changes,
  editKeys,
  onContextMenu,
}: Props): React.JSX.Element {
  const host = useRef<HTMLDivElement>(null);
  const handlers = useRef({ onHeader, onRow, onEdit, onContextMenu });
  const { theme, handle } = useGridAppearance();
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    handlers.current = { onHeader, onRow, onEdit, onContextMenu };
  });
  useEffect(() => {
    const element = host.current;
    if (!element) return;
    const props: GridProps = {
      source: tableSource({
        result,
        sort,
        changes,
        editKeys,
        rowNumbers,
        offset,
      }),
      striped,
      onEdit: (cell) => {
        handlers.current.onEdit?.(dataCell(cell, rowNumbers));
      },
      onContextMenu: (target) => {
        handlers.current.onContextMenu(dataCell(target, rowNumbers));
      },
      theme: theme.current,
      selection: { row: 0, column: rowNumbers ? 1 : 0, ranges: [] },
      onSelection: (selection) => {
        props.selection = selection;
        handlers.current.onRow(selection.row);
      },
      onHeader: (column) => {
        handlers.current.onHeader(column - (rowNumbers ? 1 : 0));
      },
      onError: (failure) => {
        setError(failure instanceof Error ? failure.message : String(failure));
      },
    };
    if (rowNumbers) props.rowHeaderColumn = 0;
    return mountCanvas(element, props, {
      theme,
      handle,
      onMounted: () => {
        initialRow(result.rows, handlers.current.onRow);
      },
    });
  }, [
    result,
    sort,
    theme,
    handle,
    offset,
    rowNumbers,
    striped,
    changes,
    editKeys,
  ]);
  return <GridFrame host={host} error={error} />;
}

function dataCell<T extends { column: number }>(
  cell: T,
  rowNumbers: boolean,
): T {
  return { ...cell, column: cell.column - (rowNumbers ? 1 : 0) };
}

function tableSource({
  result,
  sort,
  changes,
  editKeys,
  rowNumbers,
  offset,
}: Pick<
  Props,
  'result' | 'sort' | 'changes' | 'editKeys' | 'rowNumbers' | 'offset'
>): GridDataSource {
  const source = new TileSource(
    result.id,
    result.rows,
    columnsOf(result.columns, sort),
    displayWindow,
  );
  const edited = new EditedSource(source, changes, {
    keys: editKeys,
    names: result.columns.map((column) => column.name),
  });
  return rowNumbers ? new NumberedSource(edited, offset) : edited;
}

function columnWidth(type: string | null): number {
  if (type && /text|char|json|uuid/.test(type)) return 180;
  if (type && /date|time|numeric|decimal/.test(type)) return 168;
  return 96;
}

function initialRow(rows: number, onRow: (row: number) => void): void {
  if (rows > 0) onRow(0);
}

function GridFrame({
  host,
  error,
}: {
  host: React.RefObject<HTMLDivElement | null>;
  error: string | null;
}): React.JSX.Element {
  return (
    <div className={styles.grid} ref={host}>
      {error && (
        <p role="alert" className={styles.gridError}>
          Could not show rows: {error}
        </p>
      )}
    </div>
  );
}

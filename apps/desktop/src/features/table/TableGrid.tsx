// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef, useState } from 'react';
import { TileSource, gridTheme } from '@openrows/grid';
import type {
  GridColumn,
  GridHandle,
  GridProps,
  GridContextTarget,
  GridDataSource,
} from '@openrows/grid';
import type { ColumnDto, SortDto } from '@openrows/ipc';
import { useTheme } from '@openrows/ui';
import { displayWindow } from '../../results/displayWindow';
import type { ResultMeta } from '../query/run';
import { EditedSource } from './EditedSource';
import type { StagedEdit } from './editHistory';
import { NumberedSource } from './NumberedSource';
import styles from './Table.module.css';

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
  const { resolved } = useTheme();
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    handlers.current = { onHeader, onRow, onEdit, onContextMenu };
  });
  useEffect(() => {
    const element = host.current;
    if (!element) return;
    let handle: GridHandle | undefined;
    let cancelled = false;
    const props: GridProps = {
      source: tableSource({
        result,
        sort,
        changes,
        editKeys,
        rowNumbers,
        offset,
      }),
      ...(rowNumbers ? { rowHeaderColumn: 0 } : {}),
      striped,
      onEdit: (cell) => {
        handlers.current.onEdit?.(dataCell(cell, rowNumbers));
      },
      onContextMenu: (target) => {
        handlers.current.onContextMenu(dataCell(target, rowNumbers));
      },
      theme: gridTheme(resolved),
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
    import('@openrows/grid/canvas').then(
      ({ mountGrid }) => {
        if (cancelled) return;
        handle = mountGrid(element, props);
        if (result.rows > 0) handlers.current.onRow(0);
      },
      (failure: unknown) => {
        props.onError(failure);
      },
    );
    return () => {
      cancelled = true;
      handle?.destroy();
    };
  }, [result, sort, resolved, offset, rowNumbers, striped, changes, editKeys]);
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

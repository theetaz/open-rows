// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef, useState } from 'react';
import { TileSource } from '@openrows/grid';
import type { GridColumn, GridProps, GridContextTarget } from '@openrows/grid';
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
export function TableGrid(props: Props): React.JSX.Element {
  const host = useRef<HTMLDivElement>(null);
  const latest = useRef(props);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    latest.current = props;
  });
  useTableCanvas({ host, latest, setError, ...props });
  return <GridFrame host={host} error={error} />;
}

type CanvasArgs = Pick<
  Props,
  | 'result'
  | 'sort'
  | 'offset'
  | 'rowNumbers'
  | 'striped'
  | 'changes'
  | 'editKeys'
> & {
  host: React.RefObject<HTMLDivElement | null>;
  latest: React.RefObject<Props>;
  setError: (error: string) => void;
};

/** Mount the canvas once per page and view layout, not per staged change. */
function useTableCanvas({
  host,
  latest,
  setError,
  result,
  sort,
  offset,
  rowNumbers,
  striped,
  changes,
  editKeys,
}: CanvasArgs): void {
  const { theme, handle } = useGridAppearance();
  const edited = useRef<EditedSource | undefined>(undefined);
  // Staging, undo, redo and discard repaint in place: remounting would reset focus to row 0.
  useEffect(() => {
    edited.current?.stage(changes, editKeys);
  }, [changes, editKeys]);
  useEffect(() => {
    const element = host.current;
    if (!element) return;
    const overlay = editedSource({ ...latest.current, result, sort });
    edited.current = overlay;
    const props = gridProps({
      source: rowNumbers ? new NumberedSource(overlay, offset) : overlay,
      latest,
      rowNumbers,
      striped,
      theme: theme.current,
      setError,
    });
    const unmount = mountCanvas(element, props, {
      theme,
      handle,
      onMounted: () => {
        initialRow(result.rows, latest.current.onRow);
      },
    });
    return () => {
      edited.current = undefined;
      unmount();
    };
  }, [
    host,
    latest,
    setError,
    result,
    sort,
    theme,
    handle,
    offset,
    rowNumbers,
    striped,
  ]);
}

/** Grid callbacks read the latest handlers so the canvas outlives re-renders. */
function gridProps({
  latest,
  rowNumbers,
  setError,
  ...rest
}: Pick<GridProps, 'source' | 'striped' | 'theme'> &
  Pick<CanvasArgs, 'latest' | 'rowNumbers' | 'setError'>): GridProps {
  const props: GridProps = {
    ...rest,
    onEdit: (cell) => {
      latest.current.onEdit?.(dataCell(cell, rowNumbers));
    },
    onContextMenu: (target) => {
      latest.current.onContextMenu(dataCell(target, rowNumbers));
    },
    selection: { row: 0, column: rowNumbers ? 1 : 0, ranges: [] },
    onSelection: (selection) => {
      props.selection = selection;
      latest.current.onRow(selection.row);
    },
    onHeader: (column) => {
      latest.current.onHeader(column - (rowNumbers ? 1 : 0));
    },
    onError: (failure) => {
      setError(failure instanceof Error ? failure.message : String(failure));
    },
  };
  if (rowNumbers) props.rowHeaderColumn = 0;
  return props;
}

function dataCell<T extends { column: number }>(
  cell: T,
  rowNumbers: boolean,
): T {
  return { ...cell, column: cell.column - (rowNumbers ? 1 : 0) };
}

function editedSource({
  result,
  sort,
  changes,
  editKeys,
}: Pick<Props, 'result' | 'sort' | 'changes' | 'editKeys'>): EditedSource {
  const source = new TileSource(
    result.id,
    result.rows,
    columnsOf(result.columns, sort),
    displayWindow,
  );
  return new EditedSource(source, changes, {
    keys: editKeys,
    names: result.columns.map((column) => column.name),
  });
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

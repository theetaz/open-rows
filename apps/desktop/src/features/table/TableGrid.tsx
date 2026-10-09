// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef, useState } from 'react';
import { TileSource } from '@openrows/grid';
import type {
  CellRange,
  GridColumn,
  GridDataSource,
  GridHandle,
  GridProps,
  GridTheme,
  SelectionModel,
} from '@openrows/grid';
import type { ColumnDto, SortDto } from '@openrows/ipc';
import { mountCanvas } from '../../results/mountCanvas';
import { useGridAppearance } from '../../results/useGridAppearance';
import { displayWindow } from '../../results/displayWindow';
import type { ResultMeta } from '../query/run';
import { EditedSource } from './EditedSource';
import { storeColumns } from './row';
import { NumberedSource } from './NumberedSource';
import type { RowEditing } from './useRowEditing';
import { useInlineEditor } from './inline/useInlineEditor';
import type { InlineEditor } from './inline/useInlineEditor';
import { InlineEditorView } from './inline/InlineEditor';
import { useGridEditing } from './inline/useGridEditing';
import { dataRanges } from './bulk/cells';
import type { Cell } from './bulk/cells';
import { usePaste } from './bulk/usePaste';
import type { ContextTarget } from './contextMenuActions';
import styles from './Table.module.css';
import { columnAlign } from '../../results/align';

const noKeys: readonly string[] = [];
const noSelection: SelectionModel = { row: 0, column: 0, ranges: [] };

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
    ...(column.sourceType === 'bool' ? { kind: 'boolean' as const } : {}),
  }));
}

type Props = {
  result: ResultMeta;
  sort: SortDto | null;
  /** Header click on a column (sorts server-side). */
  onHeader: (column: number) => void;
  /** Row focus changed (page rows only, not staged inserts). */
  onRow: (row: number) => void;
  /** Staged changes, edit eligibility and the inline edit request. */
  editing: RowEditing;
  offset: number;
  rowNumbers: boolean;
  striped: boolean;
  /** Right-click target with the selection it belongs to (data coordinates). */
  onContextMenu: (target: ContextTarget) => void;
  /** The focused cell and ranges changed (data coordinates). */
  onSelection?: ((cell: Cell, ranges: CellRange[]) => void) | undefined;
};
type Latest = Props & { editor: InlineEditor };

/** Canvas grid over one page's result tiles; rows never enter React state. */
export function TableGrid(props: Props): React.JSX.Element {
  const host = useRef<HTMLDivElement>(null);
  const sourceRef = useRef<EditedSource | undefined>(undefined);
  const [error, setError] = useState<string | null>(null);
  const [mounted, setMounted] = useState(0);
  const { theme, handle } = useGridAppearance();
  const selectionRef = useRef<SelectionModel>(noSelection);
  const editor = useInlineEditor({
    host,
    handle,
    theme,
    source: sourceRef,
    selection: selectionRef,
    ...props,
  });
  const latest = useRef<Latest>({ ...props, editor });
  useEffect(() => {
    latest.current = { ...props, editor };
  });
  useTableCanvas({
    host,
    latest,
    setError,
    theme,
    handle,
    sourceRef,
    selectionRef,
    setMounted,
    ...props,
  });
  useGridEditing({ ...props, handle, source: sourceRef, editor, mounted });
  usePaste(host, latest, selectionRef);
  return (
    <GridFrame host={host} error={error}>
      <InlineEditorView editor={editor} />
    </GridFrame>
  );
}

type CanvasArgs = Pick<
  Props,
  'result' | 'sort' | 'offset' | 'rowNumbers' | 'striped' | 'editing'
> & {
  host: React.RefObject<HTMLDivElement | null>;
  latest: React.RefObject<Latest>;
  setError: (error: string) => void;
  theme: React.RefObject<GridTheme>;
  handle: React.RefObject<GridHandle | undefined>;
  sourceRef: React.RefObject<EditedSource | undefined>;
  selectionRef: React.RefObject<SelectionModel>;
  setMounted: React.Dispatch<React.SetStateAction<number>>;
};

/** Mount the canvas once per page and view layout, not per staged change. */
function useTableCanvas({
  host,
  latest,
  setError,
  theme,
  handle,
  sourceRef,
  selectionRef,
  setMounted,
  result,
  sort,
  offset,
  rowNumbers,
  striped,
  editing,
}: CanvasArgs): void {
  const changes = editing.history.current,
    editKeys = editing.info?.key ?? noKeys;
  // Staging, undo, redo and discard repaint in place: remounting would reset focus to row 0.
  useEffect(() => {
    sourceRef.current?.stage(changes, editKeys);
  }, [sourceRef, changes, editKeys]);
  useEffect(() => {
    const element = host.current;
    if (!element) return;
    const overlay = editedSource(latest.current, sort);
    sourceRef.current = overlay;
    const props = gridProps({
      source: rowNumbers
        ? new NumberedSource(overlay, offset, result.rows)
        : overlay,
      latest,
      rowNumbers,
      striped,
      theme: theme.current,
      setError,
      selectionRef,
    });
    selectionRef.current = props.selection;
    const unmount = mountCanvas(element, props, {
      theme,
      handle,
      onMounted: () => {
        initialRow(result.rows, latest.current.onRow);
        setMounted((count) => count + 1);
      },
    });
    return () => {
      sourceRef.current = undefined;
      unmount();
    };
  }, [
    host,
    latest,
    setError,
    setMounted,
    sourceRef,
    selectionRef,
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
  selectionRef,
  ...rest
}: Pick<GridProps, 'source' | 'striped' | 'theme'> &
  Pick<
    CanvasArgs,
    'latest' | 'rowNumbers' | 'setError' | 'selectionRef'
  >): GridProps {
  const props: GridProps = {
    ...rest,
    onEdit: (cell, text) => {
      latest.current.editor.start(dataCell(cell, rowNumbers), text);
    },
    onScroll: () => {
      latest.current.editor.reposition();
    },
    onContextMenu: (target) => {
      latest.current.onContextMenu({
        ...dataCell(target, rowNumbers),
        ranges: dataRanges(props.selection.ranges, rowNumbers),
      });
    },
    selection: { row: 0, column: rowNumbers ? 1 : 0, ranges: [] },
    onSelection: (next) => {
      props.selection = next;
      selectionRef.current = next;
      latest.current.onSelection?.(
        dataCell({ row: next.row, column: next.column }, rowNumbers),
        dataRanges(next.ranges, rowNumbers),
      );
      if (next.row < latest.current.result.rows) latest.current.onRow(next.row);
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

function editedSource(
  { result, editing }: Pick<Props, 'result' | 'editing'>,
  sort: SortDto | null,
): EditedSource {
  const columns = storeColumns(result);
  const source: GridDataSource = new TileSource(
    result.id,
    result.rows,
    columnsOf(columns, sort),
    displayWindow,
  );
  return new EditedSource(source, editing.history.current, {
    keys: editing.info?.key ?? noKeys,
    names: columns.map((column) => column.name),
    shown: result.columns.length,
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
  children,
}: {
  host: React.RefObject<HTMLDivElement | null>;
  error: string | null;
  children: React.ReactNode;
}): React.JSX.Element {
  return (
    <div className={styles.grid} ref={host}>
      {error && (
        <p role="alert" className={styles.gridError}>
          Could not show rows: {error}
        </p>
      )}
      {children}
    </div>
  );
}

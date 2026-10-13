// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef, useState } from 'react';
import { TileSource } from '@openrows/grid';
import type {
  CellRange,
  GridDataSource,
  GridHandle,
  GridProps,
  GridTheme,
  SelectionModel,
} from '@openrows/grid';
import type { SortDto } from '@openrows/ipc';
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
import { useCopyEvent } from '../../results/copy/useCopyEvent';
import { copyFailure, copySelection } from '../../results/copy/copySelection';
import type { CopyTarget } from '../../results/copy/copySelection';
import { moved, withWidth } from '../../results/columnLayout';
import { columnsOf, useLayoutSync } from './tableColumns';
import { fitColumns, useAutoFit } from './useAutoFit';
import type { ColumnLayout } from '../../results/columnLayout';
import { useGridPosition } from './gridPosition';
import type { PositionMemory } from './gridPosition';

const noKeys: readonly string[] = [];
const noSelection: SelectionModel = { row: 0, column: 0, ranges: [] };

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
  /** Column widths and pins; read when the page mounts. */
  layout: ColumnLayout;
  /** A header was resized or dragged to a new place. */
  onLayout: (update: (layout: ColumnLayout) => ColumnLayout) => void;
  /** Dialect and table of copied SQL; ⌘C copies the selection as TSV. */
  copyTarget: CopyTarget;
  /** Columns holding a foreign key: their cells show a jump arrow. */
  links?: ReadonlySet<string> | undefined;
  /** A key arrow was clicked (data coordinates). */
  onLink?: ((cell: Cell) => void) | undefined;
  /** The tab's grid position, kept across reloads. */
  position?: React.RefObject<PositionMemory> | undefined;
};
type Latest = Props & {
  editor: InlineEditor;
  fitted: React.RefObject<Map<string, number>>;
};

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
  const fitted = useRef(new Map<string, number>());
  const latest = useRef<Latest>({ ...props, editor, fitted });
  useEffect(() => {
    latest.current = { ...props, editor, fitted };
  });
  const ownPosition = useRef<PositionMemory>({
    position: null,
    restore: false,
  });
  useGridPosition({
    memory: props.position ?? ownPosition,
    handle,
    selection: selectionRef,
    mounted,
    rows: props.result.rows,
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
  useLayoutSync({ ...props, sourceRef, handle, fitted });
  useAutoFit({ ...props, sourceRef, handle, fitted, mounted });
  useCopyEvent(host, () => {
    const { result, rowNumbers, copyTarget } = latest.current;
    copySelection(
      'tsv',
      {
        result,
        ranges: dataRanges(selectionRef.current.ranges, rowNumbers).map(
          (range) => ({ ...range, r1: Math.min(range.r1, result.rows - 1) }),
        ),
      },
      copyTarget,
    ).catch((failure: unknown) => {
      latest.current.editing.fail(new Error(copyFailure(failure)));
    });
  });
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
  const mountedProps = useRef<GridProps | null>(null);
  useEffect(() => {
    if (mountedProps.current) mountedProps.current.striped = striped;
    handle.current?.refresh?.();
  }, [striped, handle]);
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
      striped: latest.current.striped,
      theme: theme.current,
      setError,
      selectionRef,
      sourceRef,
      handle,
    });
    mountedProps.current = props;
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
      mountedProps.current = null;
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
  ]);
}

/** Grid callbacks read the latest handlers so the canvas outlives re-renders. */
function gridProps({
  latest,
  rowNumbers,
  setError,
  selectionRef,
  sourceRef,
  handle,
  ...rest
}: Pick<GridProps, 'source' | 'striped' | 'theme'> &
  Pick<
    CanvasArgs,
    | 'latest'
    | 'rowNumbers'
    | 'setError'
    | 'selectionRef'
    | 'sourceRef'
    | 'handle'
  >): GridProps {
  const props: GridProps = {
    ...rest,
    ...columnHandlers({ latest, rowNumbers, sourceRef, handle }),
    onEdit: (cell, text) => {
      latest.current.editor.start(dataCell(cell, rowNumbers), text);
    },
    onFitColumns: () => {
      const current = latest.current;
      const widths = fitColumns(
        { ...current, rowNumbers, sourceRef, handle },
        true,
      );
      current.onLayout((layout) =>
        [...widths].reduce(
          (next, [name, width]) => withWidth(next, name, width),
          layout,
        ),
      );
    },
    onStepper: (cell) => {
      latest.current.editor.start(dataCell(cell, rowNumbers), undefined, true);
    },
    onScroll: () => {
      latest.current.editor.reposition();
    },
    onLink: (cell) => {
      latest.current.onLink?.(dataCell(cell, rowNumbers));
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

/** Live header resizes repaint in place; drops and final widths persist. */
function columnHandlers({
  latest,
  rowNumbers,
  sourceRef,
  handle,
}: Pick<CanvasArgs, 'latest' | 'rowNumbers' | 'sourceRef' | 'handle'>): Pick<
  GridProps,
  'onColumnResize' | 'onColumnMove'
> {
  const shift = rowNumbers ? 1 : 0;
  return {
    onColumnResize: (column, width, done) => {
      const target = sourceRef.current?.columns[column - shift];
      const name = latest.current.result.columns[column - shift]?.name;
      if (!target || name === undefined) return;
      target.width = width;
      handle.current?.relayout?.();
      if (done)
        latest.current.onLayout((layout) => withWidth(layout, name, width));
    },
    onColumnMove: (from, to) => {
      const names = latest.current.result.columns.map((column) => column.name);
      latest.current.onLayout((layout) =>
        moved(layout, names, from - shift, to - shift),
      );
    },
  };
}

function dataCell<T extends { column: number }>(
  cell: T,
  rowNumbers: boolean,
): T {
  return { ...cell, column: cell.column - (rowNumbers ? 1 : 0) };
}

function editedSource(
  {
    result,
    editing,
    layout,
    links,
  }: Pick<Props, 'result' | 'editing' | 'layout' | 'links'>,
  sort: SortDto | null,
): EditedSource {
  const columns = storeColumns(result);
  const source: GridDataSource = new TileSource(
    result.id,
    result.rows,
    columnsOf(columns, sort, layout, links),
    displayWindow,
  );
  return new EditedSource(source, editing.history.current, {
    keys: editing.info?.key ?? noKeys,
    names: columns.map((column) => column.name),
    shown: result.columns.length,
  });
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

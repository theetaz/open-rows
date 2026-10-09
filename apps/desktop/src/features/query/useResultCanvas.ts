// SPDX-License-Identifier: Apache-2.0
import { useEffect, useLayoutEffect, useRef } from 'react';
import { TileSource } from '@openrows/grid';
import type {
  GridContextTarget,
  GridHandle,
  GridProps,
  GridTheme,
  SelectionModel,
} from '@openrows/grid';
import { mountCanvas } from '../../results/mountCanvas';
import { displayWindow } from '../../results/displayWindow';
import { moved, withWidth } from '../../results/columnLayout';
import type { ColumnLayout } from '../../results/columnLayout';
import { ResultViewSource } from './ResultViewSource';
import { columnKeys, resultColumns } from './resultColumns';
import type { ResultView } from './useResultView';
import type { SelectedCell } from './cell';
import type { ResultMeta } from './run';

/** What the mounted grid's callbacks read at the moment they run. */
export type ResultLatest = {
  result: ResultMeta;
  view: ResultView;
  layout: ColumnLayout;
  changeLayout: (update: (layout: ColumnLayout) => ColumnLayout) => void;
  onCell: (cell: SelectedCell) => void;
  onContextMenu: (
    target: GridContextTarget,
    ranges: SelectionModel['ranges'],
  ) => void;
};

type Args = {
  host: React.RefObject<HTMLDivElement | null>;
  latest: React.RefObject<ResultLatest>;
  sourceRef: React.RefObject<ResultViewSource | undefined>;
  selection: React.RefObject<SelectionModel>;
  theme: React.RefObject<GridTheme>;
  handle: React.RefObject<GridHandle | undefined>;
  setError: (error: string) => void;
};

/** The grid's columns for the current layout and (applied) sort. */
function arrangement(latest: ResultLatest): ReturnType<typeof resultColumns> {
  const { result, layout, view } = latest;
  return resultColumns(result.columns, layout, view.shown ? view.sorted : null);
}

/** Inspector cell of a view selection, in result coordinates. */
function selectedCell(
  latest: ResultLatest,
  source: ResultViewSource,
  selection: SelectionModel,
): SelectedCell {
  const { result } = latest;
  const index = source.columnOf(selection.column);
  const column = result.columns[index];
  return {
    result,
    row: source.rowOf(selection.row),
    index,
    column: column?.name ?? '',
    type: column?.sourceType ?? null,
    text: source.cell(selection.row, selection.column)?.text ?? null,
  };
}

/** Header gestures: live widths in place; final widths and drops persist. */
function columnHandlers(
  { latest, handle }: Pick<Args, 'latest' | 'handle'>,
  source: ResultViewSource,
): Pick<GridProps, 'onColumnResize' | 'onColumnMove'> {
  const keyOf = (column: number): string | undefined =>
    columnKeys(latest.current.result.columns)[source.columnOf(column)];
  return {
    onColumnResize: (column, width, done) => {
      const target = source.columns[column],
        key = keyOf(column);
      if (!target || key === undefined) return;
      target.width = width;
      handle.current?.relayout?.();
      if (done)
        latest.current.changeLayout((layout) => withWidth(layout, key, width));
    },
    onColumnMove: (from, to) => {
      const shown = source.columns.map((_, column) => keyOf(column) ?? '');
      latest.current.changeLayout((layout) => moved(layout, shown, from, to));
    },
  };
}

function gridProps(args: Args, source: ResultViewSource): GridProps {
  const { latest, selection, setError } = args;
  const props: GridProps = {
    source,
    theme: args.theme.current,
    selection: { row: 0, column: 0, ranges: [] },
    ...columnHandlers(args, source),
    onSelection: (next) => {
      props.selection = next;
      selection.current = next;
      latest.current.onCell(selectedCell(latest.current, source, next));
    },
    onHeader: (column) => {
      latest.current.view.toggleSort(source.columnOf(column));
    },
    onContextMenu: (target) => {
      latest.current.onContextMenu(target, props.selection.ranges);
    },
    onError: (failure) => {
      setError(failure instanceof Error ? failure.message : String(failure));
    },
  };
  return props;
}

/** Result-order columns of the window source; the view arranges them. */
function baseSource(result: ResultMeta): TileSource {
  return new TileSource(
    result.id,
    result.rows,
    result.columns.map((column, index) => ({
      id: String(index),
      title: column.name,
      width: 88,
      pinned: false,
    })),
    displayWindow,
  );
}

/**
 * Mount the canvas once per result; streamed rows, layout, sort and filter
 * changes apply in place, keeping scroll, selection and loaded tiles.
 * `latest` must already hold this render's state (set in a layout effect).
 */
export function useResultCanvas(
  args: Args,
  { result, layout, view }: Pick<ResultLatest, 'result' | 'layout' | 'view'>,
): void {
  const { host, latest, sourceRef, selection, handle, theme, setError } = args;
  const { id } = result;
  useEffect(() => {
    const element = host.current;
    if (!element) return;
    const shown = new ResultViewSource(baseSource(latest.current.result));
    const { columns, order } = arrangement(latest.current);
    shown.arrange(columns, order);
    sourceRef.current = shown;
    const props = gridProps(
      { host, latest, sourceRef, selection, handle, theme, setError },
      shown,
    );
    const unmount = mountCanvas(element, props, { theme, handle });
    return () => {
      sourceRef.current = undefined;
      unmount();
    };
  }, [id, host, latest, sourceRef, selection, handle, theme, setError]);
  useEffect(() => {
    sourceRef.current?.grow(result.rows);
  }, [sourceRef, result.rows]);
  const sorted = view.shown ? view.sorted : null;
  useLayoutEffect(() => {
    const current = sourceRef.current;
    if (!current) return;
    const { columns, order } = arrangement(latest.current);
    current.arrange(columns, order);
    current.show(view.shown);
    handle.current?.relayout?.();
  }, [sourceRef, handle, latest, layout, sorted, view.shown]);
}

/**
 * Keep the state the mounted grid's callbacks read current; set in a layout
 * effect so `useResultCanvas` sees this render's values.
 */
export function useResultLatest(
  state: Omit<ResultLatest, 'onContextMenu'>,
  onContextMenu: ResultLatest['onContextMenu'],
): React.RefObject<ResultLatest> {
  const latest = useRef<ResultLatest>({ ...state, onContextMenu });
  useLayoutEffect(() => {
    latest.current = { ...state, onContextMenu };
  });
  return latest;
}

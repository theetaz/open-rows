// SPDX-License-Identifier: Apache-2.0
import { useMemo, useRef, useState } from 'react';
import type { GridHandle } from '@openrows/grid';
import type { EngineDto, SortDto } from '@openrows/ipc';
import {
  resetLayout,
  resultLayoutKey,
  useColumnLayout,
} from '../../results/columnLayoutStore';
import { togglePin, withWidth } from '../../results/columnLayout';
import type { ColumnLayout } from '../../results/columnLayout';
import type { CopyTarget } from '../../results/copy/copySelection';
import { resultColumnKey, storedRow } from '../../results/projectedResult';
import { useTableContext } from '../table/TableContextMenu';
import { pageRows } from '../table/bulk/bulkRow';
import { tableSettings } from '../settings/tableSettings';
import { useResultView } from './useResultView';
import { useResultEditing } from './useResultEditing';
import { useResultTools } from './useResultTools';
import { projectResult } from './projectResult';
import { changeResultView } from './resultTableView';
import { COLUMN_OPERATORS } from './columnFilters';
import { columnKeys } from './resultColumns';
import type { ResultMeta } from './run';
import type { SelectedCell } from './cell';
/** Stored query result, SQL provenance and inspector/export actions. */
export type ResultGridProps = {
  result: ResultMeta;
  engine: EngineDto;
  statement?: string | undefined;
  statementCount?: number | undefined;
  onCell: (cell: SelectedCell) => void;
  onExport?: (() => void) | undefined;
  onExportRows?: ((rows: number[]) => void) | undefined;
};

/** Shared table machinery adapted to the query result's displayed coordinates. */
export type ResultGridModel = {
  view: ReturnType<typeof useResultView>;
  edits: ReturnType<typeof useResultEditing>;
  tools: ReturnType<typeof useResultTools>;
  layout: ColumnLayout;
  changeLayout: ReturnType<typeof useColumnLayout>[1];
  hidden: readonly string[];
  setHidden: (hidden: readonly string[]) => void;
  projected: ResultMeta;
  context: ReturnType<typeof useTableContext>;
  sort: SortDto | null;
  copyTarget: CopyTarget;
  striped: boolean;
  fitAll: () => void;
  onReady: (handle: GridHandle | undefined) => void;
};
/** Compose the table tools with a result-coordinate adapter. */
export function useResultGrid(props: ResultGridProps): ResultGridModel {
  const { result, engine } = props;
  const view = useResultView(result);
  const edits = useResultEditing(
    result,
    props.statement ?? '',
    engine,
    props.statementCount,
  );
  const layoutKey = resultLayoutKey(result.columns);
  const [layout, changeLayout] = useColumnLayout(layoutKey);
  const [hidden, setHidden] = useState<readonly string[]>([]);
  const projected = useProjectedResult({
    result,
    layout,
    shown: view.shown,
    hidden,
  });
  const tools = useResultTools({
    ...edits,
    result: projected,
    original: result,
    engine,
    onCell: props.onCell,
  });
  const handle = useRef<GridHandle | undefined>(undefined);
  const fit = (column: number): void => {
    const width = handle.current?.fitWidth?.(column + 1);
    const name = resultColumnKey(projected, column);
    if (name && width !== undefined)
      changeLayout((current) => withWidth(current, name, width));
  };
  const copyTarget = {
    engine,
    table: edits.mapped
      ? { schema: edits.target.schema, name: edits.target.table }
      : null,
  };
  const context = useResultContext({
    result,
    view,
    hidden,
    setHidden,
    edits,
    tools,
    copyTarget,
    layout,
    changeLayout,
    fit,
    layoutKey,
    projected,
    onExportRows: props.onExportRows,
  });
  const sort = useResultSort(result, view);
  return {
    view,
    edits,
    layout,
    changeLayout,
    hidden,
    setHidden,
    projected,
    tools,
    onReady: (grid) => {
      handle.current = grid;
    },
    context,
    sort,
    copyTarget,
    striped: tableSettings.useValue().striped,
    fitAll: () => {
      projected.columns.forEach((_, index) => {
        fit(index);
      });
    },
  };
}

function useProjectedResult({
  result,
  layout,
  shown,
  hidden,
}: Parameters<typeof projectResult>[0]): ResultMeta {
  // Storage parsing creates fresh arrays on width-only changes. Keep the
  // coordinate adapter mounted so resizing preserves focus and selection.
  const arrangement = JSON.stringify({
    order: layout.order,
    pinned: layout.pinned,
  });
  return useMemo(
    () =>
      projectResult({
        result,
        layout: JSON.parse(arrangement) as Pick<
          ColumnLayout,
          'order' | 'pinned'
        >,
        shown,
        hidden,
      }),
    [result, arrangement, shown, hidden],
  );
}

function useResultContext(
  args: Pick<
    ResultGridModel,
    | 'view'
    | 'hidden'
    | 'setHidden'
    | 'edits'
    | 'tools'
    | 'copyTarget'
    | 'layout'
    | 'changeLayout'
    | 'projected'
  > & {
    result: ResultMeta;
    fit: (column: number) => void;
    layoutKey: string;
    onExportRows: ResultGridProps['onExportRows'];
  },
): ReturnType<typeof useTableContext> {
  const {
    result,
    view,
    hidden,
    setHidden,
    edits,
    tools,
    copyTarget,
    layout,
    changeLayout,
    fit,
    layoutKey,
    projected,
    onExportRows,
  } = args;
  return useTableContext({
    editing: edits.editing,
    jumps: tools.jumps,
    changeView: (update) => {
      changeResultView({ result, view, hidden, setHidden }, update);
    },
    quickOperators: () => COLUMN_OPERATORS,
    grid: {
      copyTarget,
      pinned: layout.pinned,
      togglePin: (name) => {
        changeLayout((current) => togglePin(current, name));
      },
      fit,
      reset: () => {
        resetLayout(layoutKey);
        setHidden([]);
      },
      exportRows: onExportRows
        ? (selection) => {
            onExportRows(
              pageRows(selection.ranges, projected.rows).map((index) =>
                storedRow(projected, index),
              ),
            );
          }
        : undefined,
    },
  });
}

function useResultSort(
  result: ResultMeta,
  view: ResultGridModel['view'],
): SortDto | null {
  const sorted = view.shown ? view.sorted : null;
  return useMemo(
    () =>
      sorted
        ? {
            column: columnKeys(result.columns)[sorted.column] ?? '',
            descending: sorted.descending,
          }
        : null,
    [result.columns, sorted],
  );
}

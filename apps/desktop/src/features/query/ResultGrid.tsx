// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef, useState } from 'react';
import { TileSource, gridTheme } from '@openrows/grid';
import type { GridColumn, GridHandle, GridProps } from '@openrows/grid';
import { resultWindow } from '@openrows/ipc';
import type { ColumnDto, WindowRequest } from '@openrows/ipc';
import { useTheme } from '@openrows/ui';
import { ArrowWorker } from '../../results/workerClient';
import type { ResultMeta } from './run';
import type { SelectedCell } from './cell';
import styles from './Query.module.css';

let shared: ArrowWorker | undefined;
/** One Arrow decode worker for every result grid, started on first use. */
function worker(): ArrowWorker {
  shared ??= new ArrowWorker();
  return shared;
}

async function load(request: WindowRequest) {
  return worker().display(await resultWindow(request), request);
}

function columnsOf(columns: readonly ColumnDto[]): GridColumn[] {
  return columns.map((column, index) => ({
    id: String(index),
    title: column.name,
    width: Math.min(320, Math.max(88, column.name.length * 8 + 40)),
    pinned: false,
  }));
}

type Props = {
  result: ResultMeta;
  onCell: (cell: SelectedCell) => void;
};

/** Canvas grid over `result_window` tiles; rows never enter React state. */
export function ResultGrid({ result, onCell }: Props): React.JSX.Element {
  const host = useRef<HTMLDivElement>(null);
  const select = useRef(onCell);
  const { resolved } = useTheme();
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    select.current = onCell;
  });
  useEffect(() => {
    const element = host.current;
    if (!element) return;
    let handle: GridHandle | undefined;
    let cancelled = false;
    const source = new TileSource(
      result.id,
      result.rows,
      columnsOf(result.columns),
      load,
    );
    const props: GridProps = {
      source,
      theme: gridTheme(resolved),
      selection: { row: 0, column: 0, ranges: [] },
      onSelection: (selection) => {
        props.selection = selection;
        const column = result.columns[selection.column];
        select.current({
          row: selection.row,
          column: column?.name ?? '',
          type: column?.sourceType ?? null,
          text: source.cell(selection.row, selection.column)?.text ?? null,
        });
      },
      onError: (failure) => {
        setError(failure instanceof Error ? failure.message : String(failure));
      },
    };
    import('@openrows/grid/canvas').then(
      ({ mountGrid }) => {
        if (!cancelled) handle = mountGrid(element, props);
      },
      (failure: unknown) => {
        props.onError(failure);
      },
    );
    return () => {
      cancelled = true;
      handle?.destroy();
    };
  }, [result, resolved]);
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

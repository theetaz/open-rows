// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef, useState } from 'react';
import { TileSource, gridTheme } from '@openrows/grid';
import type { GridColumn, GridHandle, GridProps } from '@openrows/grid';
import type { ColumnDto, SortDto } from '@openrows/ipc';
import { useTheme } from '@openrows/ui';
import { displayWindow } from '../../results/displayWindow';
import type { ResultMeta } from '../query/run';
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
    width: Math.min(320, Math.max(96, column.name.length * 8 + 56)),
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
};

/** Canvas grid over one page's result tiles; rows never enter React state. */
export function TableGrid({
  result,
  sort,
  onHeader,
  onRow,
}: Props): React.JSX.Element {
  const host = useRef<HTMLDivElement>(null);
  const handlers = useRef({ onHeader, onRow });
  const { resolved } = useTheme();
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    handlers.current = { onHeader, onRow };
  });
  useEffect(() => {
    const element = host.current;
    if (!element) return;
    let handle: GridHandle | undefined;
    let cancelled = false;
    const props: GridProps = {
      source: new TileSource(
        result.id,
        result.rows,
        columnsOf(result.columns, sort),
        displayWindow,
      ),
      theme: gridTheme(resolved),
      selection: { row: 0, column: 0, ranges: [] },
      onSelection: (selection) => {
        props.selection = selection;
        handlers.current.onRow(selection.row);
      },
      onHeader: (column) => {
        handlers.current.onHeader(column);
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
  }, [result, sort, resolved]);
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

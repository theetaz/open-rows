// SPDX-License-Identifier: Apache-2.0
import { useRef, useState } from 'react';
import { Button, Icon, TextField } from '@openrows/ui';
import type { SelectionModel } from '@openrows/grid';
import type { EngineDto } from '@openrows/ipc';
import { useGridAppearance } from '../../results/useGridAppearance';
import {
  resetLayout,
  resultLayoutKey,
  useColumnLayout,
} from '../../results/columnLayoutStore';
import { copyFailure, copySelection } from '../../results/copy/copySelection';
import type { CopyFormat } from '../../results/copy/formats';
import { useCopyEvent } from '../../results/copy/useCopyEvent';
import { ResultMenus } from './ResultMenus';
import type { Menu } from './ResultMenus';
import type { ResultViewSource } from './ResultViewSource';
import { useResultCanvas, useResultLatest } from './useResultCanvas';
import { useResultView } from './useResultView';
import type { ResultView } from './useResultView';
import type { ResultMeta } from './run';
import type { SelectedCell } from './cell';
import styles from './Query.module.css';

type Props = {
  result: ResultMeta;
  /** Engine of the connection: quoting for copied SQL. */
  engine: EngineDto;
  onCell: (cell: SelectedCell) => void;
  /** Export… of the selected rows (stored positions); omitted while unavailable. */
  onExportRows?: ((rows: number[]) => void) | undefined;
};

const noSelection: SelectionModel = { row: 0, column: 0, ranges: [] };

/**
 * Canvas grid over `result_window` tiles, shown while rows stream in; rows
 * never enter React state. Headers sort and the filter field narrows the
 * loaded rows client-side; columns resize, move and pin like TablePlus.
 */
export function ResultGrid({
  result,
  engine,
  onCell,
  onExportRows,
}: Props): React.JSX.Element {
  const host = useRef<HTMLDivElement>(null);
  const sourceRef = useRef<ResultViewSource | undefined>(undefined);
  const selection = useRef<SelectionModel>(noSelection);
  const { theme, handle } = useGridAppearance();
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [menu, setMenu] = useState<Menu | null>(null);
  const layoutKey = resultLayoutKey(result.columns);
  const [layout, changeLayout] = useColumnLayout(layoutKey);
  const view = useResultView(result);
  const latest = useResultLatest(
    { result, view, layout, changeLayout, onCell },
    (target, ranges) => {
      const column = sourceRef.current?.columnOf(target.column);
      setMenu({ ...target, ranges, stored: column ?? target.column });
    },
  );
  useResultCanvas(
    { host, latest, sourceRef, selection, handle, theme, setError },
    { result, layout, view },
  );
  const copy = useResultCopy({
    host,
    result,
    engine,
    sourceRef,
    selection,
    setNotice,
  });
  return (
    <div className={styles.resultView}>
      <ResultTools view={view} notice={notice} />
      <div className={styles.grid} ref={host}>
        {error && (
          <p role="alert" className={styles.gridError}>
            Could not show rows: {error}
          </p>
        )}
      </div>
      <ResultMenus
        menu={menu}
        result={result}
        view={view}
        pinned={layout.pinned}
        actions={{
          copy: copy.selection,
          copyText: copy.text,
          exportRows: onExportRows
            ? (ranges) => {
                const shown = sourceRef.current;
                if (shown)
                  onExportRows(selectedRows(ranges, (row) => shown.rowOf(row)));
              }
            : undefined,
          changeLayout,
          fitWidth: (column) => handle.current?.fitWidth?.(column),
          reset: () => {
            resetLayout(layoutKey);
          },
          close: () => {
            setMenu(null);
          },
        }}
      />
    </div>
  );
}

/** Stored positions of the rows a selection covers, ascending. */
function selectedRows(
  ranges: SelectionModel['ranges'],
  rowOf: (row: number) => number | undefined,
): number[] {
  const rows = new Set<number>();
  for (const range of ranges)
    for (let row = range.r0; row <= range.r1; row += 1) {
      const stored = rowOf(row);
      if (stored !== undefined) rows.add(stored);
    }
  return [...rows].sort((a, b) => a - b);
}

/** Copy the selection (also on ⌘C) or plain text, reporting failures. */
function useResultCopy({
  host,
  result,
  engine,
  sourceRef,
  selection,
  setNotice,
}: {
  host: React.RefObject<HTMLDivElement | null>;
  result: ResultMeta;
  engine: EngineDto;
  sourceRef: React.RefObject<ResultViewSource | undefined>;
  selection: React.RefObject<SelectionModel>;
  setNotice: (notice: string | null) => void;
}): {
  selection: (format: CopyFormat, ranges: SelectionModel['ranges']) => void;
  text: (text: string) => void;
} {
  const report = (pending: Promise<void>): void => {
    pending.then(
      () => {
        setNotice(null);
      },
      (failure: unknown) => {
        setNotice(copyFailure(failure));
      },
    );
  };
  const copy = (format: CopyFormat, ranges: SelectionModel['ranges']): void => {
    const shown = sourceRef.current;
    if (!shown) return;
    report(
      copySelection(
        format,
        {
          result,
          ranges,
          rowOf: (row) => shown.rowOf(row),
          columnOf: (column) => shown.columnOf(column),
        },
        { engine, table: null },
      ),
    );
  };
  useCopyEvent(host, () => {
    copy('tsv', selection.current.ranges);
  });
  return {
    selection: copy,
    text: (text) => {
      report(navigator.clipboard.writeText(text));
    },
  };
}

/** The filter field and the sort/filter status of a result. */
function ResultTools({
  view,
  notice,
}: {
  view: ResultView;
  notice: string | null;
}): React.JSX.Element {
  const status = notice ?? view.status;
  const columns = view.columnFilters.length;
  return (
    <div className={styles.resultTools}>
      <TextField
        aria-label="Filter rows"
        placeholder="Filter rows"
        controlSize="compact"
        className={styles.resultFilter}
        leadingIcon={<Icon name="search" />}
        value={view.needle}
        onChange={(event) => {
          view.setNeedle(event.target.value);
        }}
      />
      {columns > 0 && (
        <Button size="sm" variant="ghost" onClick={view.clearColumnFilters}>
          Clear column filters ({columns})
        </Button>
      )}
      {status && (
        <span role="status" className={styles.resultStatus}>
          {status}
        </span>
      )}
    </div>
  );
}

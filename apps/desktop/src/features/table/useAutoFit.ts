// SPDX-License-Identifier: Apache-2.0
import { useEffect } from 'react';
import type { GridHandle } from '@openrows/grid';
import type { ColumnLayout } from '../../results/columnLayout';
import type { ResultMeta } from '../query/run';
import type { EditedSource } from './EditedSource';
import { resultColumnKey } from '../../results/projectedResult';

type Args = {
  result: ResultMeta;
  layout: ColumnLayout;
  rowNumbers: boolean;
  sourceRef: React.RefObject<EditedSource | undefined>;
  handle: React.RefObject<GridHandle | undefined>;
  /** Auto-fitted widths by column name, re-applied after layout syncs. */
  fitted: React.RefObject<Map<string, number>>;
  mounted: number;
};

/**
 * Fit columns to their loaded values (capped) and return the widths by name.
 * `all` also refits columns whose width the user stored.
 */
export function fitColumns(
  args: Omit<Args, 'mounted'>,
  all: boolean,
): Map<string, number> {
  const { result, layout, sourceRef, handle, fitted } = args;
  const shift = args.rowNumbers ? 1 : 0;
  const columns = sourceRef.current?.columns;
  const grid = handle.current;
  const widths = new Map<string, number>();
  if (!columns || !grid?.fitWidth) return widths;
  result.columns.forEach((_, index) => {
    const name = resultColumnKey(result, index);
    const column = columns[index];
    const kept = layout.widths[name] !== undefined || fitted.current.has(name);
    if (!column || (!all && kept)) return;
    const width = grid.fitWidth?.(index + shift);
    if (width === undefined) return;
    column.width = width;
    widths.set(name, width);
    fitted.current.set(name, width);
  });
  grid.relayout?.();
  return widths;
}

/** Size columns to their content once each page's first rows arrive, like TablePlus. */
export function useAutoFit(args: Args): void {
  const { mounted, sourceRef } = args;
  const latest = { ...args };
  useEffect(() => {
    const source = sourceRef.current;
    if (!mounted || !source) return;
    let done = false;
    const fit = (): void => {
      if (done || source.rows === 0 || !source.cell(0, 0)) return;
      done = true;
      fitColumns(latest, false);
    };
    const stop = source.subscribe(fit);
    fit();
    return () => {
      done = true;
      stop();
    };
    // Fit once per mounted page; later layout changes keep the fitted widths.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [mounted, sourceRef]);
}

// SPDX-License-Identifier: Apache-2.0
import { useEffect, useMemo, useState } from 'react';
import { displayWindow } from '../../results/displayWindow';
import { columnAlign } from '../../results/align';
import { LocalRows, localLimit, viewRows } from './localRows';
import type { RowOrder } from './localRows';
import type { ResultMeta } from './run';

/** Typing pauses this long before the filter runs over loaded rows. */
const FILTER_DELAY_MS = 150;

/** Client-side sort and quick filter of one query result (FR-GRID-04). */
export type ResultView = {
  /** Filter text as typed. */
  needle: string;
  setNeedle: (needle: string) => void;
  sorted: RowOrder | null;
  /** Sort by a result column, or clear the sort with `null`. */
  sortBy: (order: RowOrder | null) => void;
  /** Header click: ascending, then descending, then unsorted. */
  toggleSort: (column: number) => void;
  /** Loaded rows and the ones shown, or `null` for every row in order. */
  shown: { local: LocalRows; rows: Int32Array } | null;
  /** Loading, a match count or why sort and filter are unavailable. */
  status: string | null;
};

function nextOrder(current: RowOrder | null, column: number): RowOrder | null {
  if (current?.column !== column) return { column, descending: false };
  return current.descending ? null : { column, descending: true };
}

/** Load every row once, when a sort or filter first needs them. */
function useLocalRows(
  result: ResultMeta,
  wanted: boolean,
): { local: LocalRows | null; failure: string | null } {
  const [local, setLocal] = useState<LocalRows | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const allowed = localLimit(result) === null;
  useEffect(() => {
    if (!wanted || !allowed || local) return;
    let alive = true;
    LocalRows.load(result, displayWindow).then(
      (rows) => {
        if (alive) setLocal(rows);
      },
      (error: unknown) => {
        if (alive)
          setFailure(error instanceof Error ? error.message : String(error));
      },
    );
    return () => {
      alive = false;
    };
  }, [wanted, allowed, local, result]);
  return { local, failure };
}

/** The current filter text after a short pause in typing. */
function useSettled(needle: string): string {
  const [settled, setSettled] = useState(needle);
  useEffect(() => {
    const timer = window.setTimeout(() => {
      setSettled(needle);
    }, FILTER_DELAY_MS);
    return () => {
      window.clearTimeout(timer);
    };
  }, [needle]);
  return settled;
}

function statusOf(
  result: ResultMeta,
  wanted: boolean,
  {
    local,
    failure,
    shown,
  }: Pick<ReturnType<typeof useLocalRows>, 'local' | 'failure'> & {
    shown: ResultView['shown'];
  },
  filtered: boolean,
): string | null {
  if (!wanted) return null;
  if (failure) return `Could not load rows: ${failure}`;
  const limit = localLimit(result);
  if (limit) return limit;
  if (!local) return 'Loading rows…';
  if (filtered && shown)
    return `${shown.rows.length.toLocaleString('en-US')} of ${result.rows.toLocaleString('en-US')} rows`;
  return null;
}

/** Sort and filter state of a result grid, applied over its loaded rows. */
export function useResultView(result: ResultMeta): ResultView {
  const [needle, setNeedle] = useState('');
  const [sorted, setSorted] = useState<RowOrder | null>(null);
  const query = useSettled(needle).trim();
  const wanted = query !== '' || sorted !== null;
  const { local, failure } = useLocalRows(result, wanted);
  const shown = useMemo(() => {
    if (!local) return null;
    const rows = viewRows(
      local,
      query,
      sorted,
      (column) =>
        columnAlign(result.columns[column]?.sourceType ?? null) === 'end',
    );
    return rows ? { local, rows } : null;
  }, [local, query, sorted, result.columns]);
  return {
    needle,
    setNeedle,
    sorted,
    sortBy: setSorted,
    toggleSort: (column) => {
      setSorted((current) => nextOrder(current, column));
    },
    shown,
    status: statusOf(result, wanted, { local, failure, shown }, query !== ''),
  };
}

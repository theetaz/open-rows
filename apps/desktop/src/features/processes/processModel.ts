// SPDX-License-Identifier: Apache-2.0
import type { ProcessDto } from '@openrows/ipc';
import type { RowField } from '../table/row';

/** A sortable process-list column. */
type ProcessColumn = {
  key: keyof ProcessDto;
  title: string;
  width: number;
};

/** Columns of the list, left to right. */
export const PROCESS_COLUMNS: readonly ProcessColumn[] = [
  { key: 'id', title: 'PID', width: 80 },
  { key: 'user', title: 'User', width: 110 },
  { key: 'database', title: 'Database', width: 120 },
  { key: 'client', title: 'Client', width: 150 },
  { key: 'application', title: 'Application', width: 160 },
  { key: 'state', title: 'State', width: 120 },
  { key: 'wait', title: 'Waiting for', width: 140 },
  { key: 'durationMs', title: 'Duration', width: 90 },
  { key: 'queryStart', title: 'Query start', width: 190 },
  { key: 'query', title: 'Query', width: 420 },
];

/** The sorted column and direction. */
export type ProcessSort = { col: number; descending: boolean };

/** Like the server's order: the longest running first. */
export const DEFAULT_SORT: ProcessSort = {
  col: PROCESS_COLUMNS.findIndex((column) => column.key === 'durationMs'),
  descending: true,
};

/** Auto-refresh intervals in seconds; `0` is off. */
export const REFRESH_INTERVALS = ['0', '2', '5', '10'] as const;
export type RefreshInterval = (typeof REFRESH_INTERVALS)[number];

/** `850 ms`, `12.4 s`, `3 m 05 s`, `2 h 07 m`; empty when unknown. */
export function formatDuration(ms: number | null): string {
  if (ms === null) return '';
  if (ms < 1000) return `${String(Math.round(ms))} ms`;
  const seconds = ms / 1000;
  if (seconds < 60) return `${seconds.toFixed(1)} s`;
  const whole = Math.floor(seconds);
  const pad = (value: number): string => String(value).padStart(2, '0');
  if (whole < 3600)
    return `${String(Math.floor(whole / 60))} m ${pad(whole % 60)} s`;
  return `${String(Math.floor(whole / 3600))} h ${pad(Math.floor(whole / 60) % 60)} m`;
}

/** The display text of one cell; the query is shown on one line. */
export function cellText(process: ProcessDto, key: keyof ProcessDto): string {
  if (key === 'durationMs') return formatDuration(process.durationMs);
  if (key === 'query') return (process.query ?? '').replace(/\s+/g, ' ').trim();
  const value = process[key];
  return value === null ? '' : String(value);
}

/** Processes whose text contains `needle` (case-insensitive) in any column. */
export function filterProcesses(
  processes: readonly ProcessDto[],
  needle: string,
): ProcessDto[] {
  const search = needle.trim().toLowerCase();
  if (!search) return [...processes];
  return processes.filter((process) =>
    PROCESS_COLUMNS.some(({ key }) =>
      cellText(process, key).toLowerCase().includes(search),
    ),
  );
}

function compareValues(
  left: ProcessDto[keyof ProcessDto],
  right: ProcessDto[keyof ProcessDto],
): number {
  if (left === right) return 0;
  if (left === null) return 1;
  if (right === null) return -1;
  if (typeof left === 'number' && typeof right === 'number')
    return left - right;
  return String(left).localeCompare(String(right));
}

/** A sorted copy; empty values last in either direction, ties by PID. */
export function sortProcesses(
  processes: readonly ProcessDto[],
  sort: ProcessSort,
): ProcessDto[] {
  const key = PROCESS_COLUMNS[sort.col]?.key ?? 'id';
  const sign = sort.descending ? -1 : 1;
  return [...processes].sort((left, right) => {
    const a = left[key];
    const b = right[key];
    if (a === null || b === null)
      return compareValues(a, b) || left.id - right.id;
    return sign * compareValues(a, b) || left.id - right.id;
  });
}

/** Click on a header: sort by it, or flip the direction when it is sorted. */
export function nextSort(current: ProcessSort, col: number): ProcessSort {
  if (current.col === col) return { col, descending: !current.descending };
  return { col, descending: PROCESS_COLUMNS[col]?.key === 'durationMs' };
}

/** Inspector fields of one process; the query keeps its full text. */
export function processFields(process: ProcessDto): RowField[] {
  return PROCESS_COLUMNS.map(({ key, title }, index) => {
    const text =
      key === 'query' ? (process.query ?? '') : cellText(process, key);
    const missing = process[key] === null;
    return {
      name: title,
      index,
      type: null,
      text,
      state: missing ? 'null' : text ? 'normal' : 'empty',
    };
  });
}

/** Who the process is, for confirmations and the inspector caption. */
export function processLabel(process: ProcessDto): string {
  const owner = [process.user, process.application]
    .filter((part): part is string => Boolean(part))
    .join(', ');
  return owner
    ? `process ${String(process.id)} (${owner})`
    : `process ${String(process.id)}`;
}

// SPDX-License-Identifier: Apache-2.0
import type { RunState } from './run';

const count = (value: number, noun: string): string =>
  `${value.toLocaleString()} ${noun}${value === 1 ? '' : 's'}`;

/** Rows returned, rows affected and notices of a run, e.g. `120 rows · 3 rows affected`. */
function totals(run: RunState): string[] {
  const parts: string[] = [];
  const rows = run.results.reduce((sum, result) => sum + result.rows, 0);
  if (run.results.length) parts.push(count(rows, 'row'));
  if (run.results.some((result) => result.truncated))
    parts.push('truncated at the row limit');
  if (run.affected !== null)
    parts.push(`${count(run.affected, 'row')} affected`);
  const notices = run.messages.filter((m) => m.level !== 'error').length;
  if (notices) parts.push(count(notices, 'notice'));
  return parts;
}

/** One-line summary for the query status bar. */
export function statusText(run: RunState): string {
  const elapsed =
    run.elapsedMs === null ? [] : [`${run.elapsedMs.toLocaleString()} ms`];
  const total = run.statements.length;
  switch (run.status) {
    case 'idle':
      return 'Ready';
    case 'confirm':
      return 'Waiting for confirmation';
    case 'blocked':
      return 'Blocked: read-only connection';
    case 'starting':
    case 'running':
      return [
        total > 1
          ? `Running statement ${String(Math.min(run.done + 1, total))} of ${String(total)}`
          : 'Running…',
        ...totals(run),
      ].join(' · ');
    case 'completed':
      return [
        total > 1 ? `${String(total)} statements` : 'Done',
        ...totals(run),
        ...elapsed,
      ].join(' · ');
    case 'cancelled':
      return ['Cancelled', ...totals(run), ...elapsed].join(' · ');
    case 'failed':
      return [failure(run), ...elapsed].join(' · ');
  }
}

/** `Failed in statement 2: <message> (SQLSTATE 42P01)` from the last error. */
function failure(run: RunState): string {
  const error = run.messages.filter((m) => m.level === 'error').at(-1);
  if (!error) return 'Failed';
  const where =
    run.statements.length > 1
      ? ` in statement ${String(error.statement + 1)}`
      : '';
  const code = error.code ? ` (SQLSTATE ${error.code})` : '';
  return `Failed${where}: ${error.text}${code}`;
}

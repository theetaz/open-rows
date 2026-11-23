// SPDX-License-Identifier: Apache-2.0
import type { RunScope } from './request';
import type { RunState } from './run';
import { t, tn } from './lazyMessages';

/** Rows returned, rows affected and notices of a run, e.g. `120 rows · 3 rows affected`. */
function totals(run: RunState): string[] {
  const parts: string[] = [];
  const rows = run.results.reduce((sum, result) => sum + result.rows, 0);
  if (run.results.length)
    parts.push(tn('rowCount', rows, { count: rows.toLocaleString() }));
  if (run.results.some((result) => result.truncated))
    parts.push(t('truncatedRowLimit'));
  if (run.affected !== null)
    parts.push(
      tn('rowsAffected', run.affected, {
        count: run.affected.toLocaleString(),
      }),
    );
  const notices = run.messages.filter((m) => m.level !== 'error').length;
  if (notices)
    parts.push(tn('noticeCount', notices, { count: notices.toLocaleString() }));
  return parts;
}

/** One-line summary for the query status bar; before a run, how to start one. */
export function statusText(run: RunState, runDefault: RunScope): string {
  const elapsed =
    run.elapsedMs === null ? [] : [`${run.elapsedMs.toLocaleString()} ms`];
  const total = run.statements.length;
  switch (run.status) {
    case 'idle':
      return runDefault === 'all' ? t('readyHintAll') : t('readyHint');
    case 'confirm':
      return t('waitingConfirmation');
    case 'parameters':
      return t('waitingParameterValues');
    case 'blocked':
      return t('blockedReadOnly');
    case 'starting':
    case 'running':
      return [
        total > 1
          ? t('runningStatementOf', {
              done: String(Math.min(run.done + 1, total)),
              total: String(total),
            })
          : t('running'),
        ...totals(run),
      ].join(' · ');
    case 'completed':
      return [
        total > 1 ? t('statementTotal', { total: String(total) }) : t('done'),
        ...totals(run),
        ...elapsed,
      ].join(' · ');
    case 'cancelled':
      return [t('cancelled'), ...totals(run), ...elapsed].join(' · ');
    case 'failed':
      return [failure(run), ...elapsed].join(' · ');
  }
}

/** `Failed in statement 2: <message> (SQLSTATE 42P01)` from the last error. */
function failure(run: RunState): string {
  const error = run.messages.filter((m) => m.level === 'error').at(-1);
  if (!error) return t('failed');
  const text = error.code
    ? t('textWithSqlstate', { text: error.text, code: error.code })
    : error.text;
  return run.statements.length > 1
    ? t('failedInStatement', { statement: String(error.statement + 1), text })
    : t('failedText', { text });
}

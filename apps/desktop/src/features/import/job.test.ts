// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import type { ImportSummaryDto } from '@openrows/ipc';
import { idleJob, importReducer, isBusy, rowCount } from './job';

const summary: ImportSummaryDto = {
  rows: 3,
  inserted: 2,
  skipped: 0,
  failed: 1,
  conflicts: null,
  errors: [{ line: 3, column: 'id', message: '"x" is not a valid integer' }],
};

it('tracks a job from start through progress to its summary', () => {
  let job = importReducer(idleJob, { type: 'start', mode: 'import' });
  expect(isBusy(job)).toBe(true);
  job = importReducer(job, {
    type: 'event',
    event: {
      type: 'progress',
      rows: 10,
      inserted: 9,
      failed: 1,
      fraction: 0.5,
      rowsPerSecond: null,
    },
  });
  expect(job).toMatchObject({
    status: 'running',
    importId: '',
    progress: { rows: 10, fraction: 0.5, rowsPerSecond: 0 },
  });
  job = importReducer(job, { type: 'started', importId: 'import-1' });
  expect(job).toMatchObject({ status: 'running', importId: 'import-1' });
  job = importReducer(job, {
    type: 'event',
    event: { type: 'finished', summary },
  });
  expect(job).toEqual({ status: 'finished', mode: 'import', summary });
  expect(isBusy(job)).toBe(false);
  expect(importReducer(job, { type: 'started', importId: 'late' })).toBe(job);
  expect(importReducer(job, { type: 'reset' })).toBe(idleJob);
});

it('keeps the mode through failures and cancels', () => {
  const checking = importReducer(idleJob, { type: 'start', mode: 'check' });
  expect(
    importReducer(checking, { type: 'rejected', message: 'No such table' }),
  ).toEqual({
    status: 'failed',
    mode: 'check',
    message: 'No such table',
    summary: null,
  });
  const failed = importReducer(checking, {
    type: 'event',
    event: {
      type: 'failed',
      error: { kind: 'server', message: 'duplicate key', code: '23505' },
      summary,
    },
  });
  expect(failed).toMatchObject({ status: 'failed', message: 'duplicate key' });
  const cancelled = importReducer(checking, {
    type: 'event',
    event: { type: 'cancelled', summary },
  });
  expect(cancelled).toMatchObject({ status: 'cancelled', mode: 'check' });
  expect(rowCount(1)).toBe('1 row');
  expect(rowCount(2500)).toBe('2,500 rows');
  expect(rowCount(null)).toBe('0 rows');
});

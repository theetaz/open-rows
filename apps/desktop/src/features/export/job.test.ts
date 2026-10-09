// SPDX-License-Identifier: Apache-2.0
import { describe, expect, it } from 'vitest';
import { idleJob, isBusy, jobReducer } from './job';
import type { ExportJob, JobAction } from './job';

const run = (actions: readonly JobAction[]): ExportJob =>
  actions.reduce(jobReducer, idleJob);

describe('export job', () => {
  it('runs from the save panel to a finished notice with the row count', () => {
    const started = run([
      { type: 'start' },
      {
        type: 'reply',
        reply: { status: 'started', exportId: 'e1', fileName: 'users.csv' },
      },
      { type: 'event', event: { type: 'progress', rows: 500 } },
    ]);
    expect(started).toEqual({
      status: 'running',
      exportId: 'e1',
      fileName: 'users.csv',
      rows: 500,
    });
    expect(isBusy(started)).toBe(true);
    const done = jobReducer(started, {
      type: 'event',
      event: { type: 'finished', rows: 1200, path: '/tmp/out/users.csv' },
    });
    expect(done).toEqual({
      status: 'finished',
      fileName: 'users.csv',
      path: '/tmp/out/users.csv',
      rows: 1200,
    });
    expect(isBusy(done)).toBe(false);
  });

  it('keeps events that overtake the reply', () => {
    const early = run([
      { type: 'start' },
      { type: 'event', event: { type: 'progress', rows: 10 } },
      {
        type: 'reply',
        reply: { status: 'started', exportId: 'e2', fileName: 'r.json' },
      },
    ]);
    expect(early).toMatchObject({ exportId: 'e2', rows: 10 });
    const finishedFirst = run([
      { type: 'start' },
      {
        type: 'event',
        event: { type: 'finished', rows: 3, path: 'C:\\out\\r.json' },
      },
      {
        type: 'reply',
        reply: { status: 'started', exportId: 'e3', fileName: 'r.json' },
      },
    ]);
    expect(finishedFirst).toMatchObject({ status: 'finished', rows: 3 });
  });

  it('returns to the options when the panel is dismissed and reports failures', () => {
    expect(
      run([
        { type: 'start' },
        { type: 'reply', reply: { status: 'dismissed' } },
      ]),
    ).toEqual(idleJob);
    expect(
      run([
        { type: 'start' },
        {
          type: 'event',
          event: {
            type: 'failed',
            error: { kind: 'storage', message: 'Disk full', code: null },
          },
        },
      ]),
    ).toEqual({ status: 'failed', message: 'Disk full' });
    expect(
      run([{ type: 'start' }, { type: 'rejected', message: 'Invalid' }]),
    ).toEqual({ status: 'failed', message: 'Invalid' });
    expect(
      run([{ type: 'start' }, { type: 'event', event: { type: 'cancelled' } }]),
    ).toEqual({ status: 'cancelled' });
  });
});

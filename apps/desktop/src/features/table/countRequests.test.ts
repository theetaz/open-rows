// SPDX-License-Identifier: Apache-2.0
import { cancelQuery, tableCount } from '@openrows/ipc';
import type { CountReply, TableQueryDto } from '@openrows/ipc';
import { expect, it, vi } from 'vitest';
import { countRequests } from './countRequests';

vi.mock('@openrows/ipc', () => ({
  cancelQuery: vi.fn(() => Promise.resolve()),
  tableCount: vi.fn(),
}));

const request = { sessionId: 'session', query: {} as TableQueryDto };

function pending() {
  let resolve!: (reply: CountReply) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<CountReply>((done, fail) => {
    resolve = done;
    reject = fail;
  });
  return { promise, resolve, reject };
}

it('cancels the previous count and keeps its late reply from replacing the newer one', async () => {
  const first = pending();
  const second = pending();
  vi.mocked(tableCount)
    .mockReturnValueOnce(first.promise)
    .mockReturnValueOnce(second.promise);
  const send = vi.fn();
  const counts = countRequests(send);
  counts.start('tab', 'first filter', request);
  const firstTicket = vi.mocked(tableCount).mock.lastCall?.[0].ticket;
  counts.start('tab', 'second filter', request);
  const secondTicket = vi.mocked(tableCount).mock.lastCall?.[0].ticket;
  expect(cancelQuery).toHaveBeenCalledExactlyOnceWith(firstTicket);
  counts.cancel('tab');
  expect(cancelQuery).toHaveBeenLastCalledWith(secondTicket);
  second.resolve({ kind: 'counted', rows: 7 });
  await second.promise;
  first.resolve({ kind: 'counted', rows: 100 });
  await first.promise;
  expect(send).toHaveBeenCalledTimes(3);
  expect(send).toHaveBeenLastCalledWith('tab', {
    status: 'done',
    key: 'second filter',
    rows: 7,
  });
});

it('does not recreate a released table when its count completes', async () => {
  const reply = pending();
  vi.mocked(tableCount).mockReturnValueOnce(reply.promise);
  const send = vi.fn();
  const counts = countRequests(send);
  counts.start('tab', 'filter', request);
  const ticket = vi.mocked(tableCount).mock.lastCall?.[0].ticket;
  counts.release('tab');
  expect(cancelQuery).toHaveBeenCalledExactlyOnceWith(ticket);
  reply.resolve({ kind: 'counted', rows: 100 });
  await reply.promise;
  expect(send).toHaveBeenCalledOnce();
});

it('cancels every owned table count on workspace cleanup and ignores late errors', async () => {
  const first = pending();
  const second = pending();
  vi.mocked(tableCount)
    .mockReturnValueOnce(first.promise)
    .mockReturnValueOnce(second.promise);
  const send = vi.fn();
  const counts = countRequests(send);
  counts.start('one', 'filter', request);
  counts.start('two', 'filter', request);
  counts.clear();
  expect(cancelQuery).toHaveBeenCalledTimes(2);
  first.reject(new Error('old failure'));
  second.resolve({ kind: 'cancelled' });
  await Promise.allSettled([first.promise, second.promise]);
  expect(send).toHaveBeenCalledTimes(2);
});

it('reports a current cancellation or failure and leaves completed tickets alone', async () => {
  const reply = pending();
  vi.mocked(tableCount).mockReturnValueOnce(reply.promise);
  const send = vi.fn();
  const counts = countRequests(send);
  counts.start('tab', 'filter', request);
  counts.cancel('tab');
  reply.resolve({ kind: 'cancelled' });
  await reply.promise;
  expect(send).toHaveBeenLastCalledWith('tab', { status: 'idle' });
  const failure = pending();
  vi.mocked(tableCount).mockReturnValueOnce(failure.promise);
  counts.start('tab', 'filter', request);
  failure.reject(new Error('count failed'));
  await Promise.allSettled([failure.promise]);
  expect(send).toHaveBeenLastCalledWith('tab', {
    status: 'error',
    key: 'filter',
    message: 'count failed',
  });
  counts.clear();
  expect(cancelQuery).toHaveBeenCalledOnce();
});

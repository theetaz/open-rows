// SPDX-License-Identifier: Apache-2.0
import { expect, it, vi } from 'vitest';
import { cancelQuery, executeQuery, releaseResults } from './queries';

const mockInvoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({
  invoke: mockInvoke,
  Channel: class {
    onmessage: (event: unknown) => void = () => {};
  },
}));

it('passes the request and a channel that forwards events', async () => {
  const reply = { kind: 'started', queryId: 'q', statements: [] };
  mockInvoke.mockResolvedValue(reply);
  const events = vi.fn();
  const request = {
    sessionId: 's',
    sql: 'SELECT 1',
    mode: 'all' as const,
    cursor: null,
    confirmToken: null,
  };
  expect(await executeQuery(request, events)).toEqual(reply);
  const [command, args] = mockInvoke.mock.calls[0] as [
    string,
    { request: unknown; onEvent: { onmessage: (event: unknown) => void } },
  ];
  expect(command).toBe('query_execute');
  expect(args.request).toEqual(request);
  args.onEvent.onmessage({ type: 'finished' });
  expect(events).toHaveBeenCalledWith({ type: 'finished' });
});

it('rejects typed errors and skips empty releases', async () => {
  mockInvoke.mockRejectedValue({
    kind: 'invalid',
    message: 'There is no SQL statement to run',
    code: null,
  });
  await expect(
    executeQuery(
      {
        sessionId: 's',
        sql: '',
        mode: 'all',
        cursor: null,
        confirmToken: null,
      },
      vi.fn(),
    ),
  ).rejects.toMatchObject({ kind: 'invalid' });
  mockInvoke.mockReset().mockResolvedValue(null);
  await releaseResults([]);
  expect(mockInvoke).not.toHaveBeenCalled();
  await releaseResults(['r1']);
  await cancelQuery('q');
  expect(mockInvoke.mock.calls).toEqual([
    ['results_release', { resultIds: ['r1'] }],
    ['query_cancel', { queryId: 'q' }],
  ]);
});

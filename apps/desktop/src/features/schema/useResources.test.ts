// SPDX-License-Identifier: Apache-2.0
import { act, renderHook } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import { useResources } from './useResources';
import type { Fetch } from './useResources';

/** A fetch whose replies the test releases one by one. */
function controlled(): {
  fetch: Fetch<string> & ReturnType<typeof vi.fn>;
  reply: (data: string, stale: boolean) => Promise<void>;
  fail: (message: string) => Promise<void>;
} {
  const pending: {
    resolve: (value: { data: string; stale: boolean }) => void;
    reject: (error: Error) => void;
  }[] = [];
  const fetch = vi.fn(
    () =>
      new Promise<{ data: string; stale: boolean }>((resolve, reject) => {
        pending.push({ resolve, reject });
      }),
  );
  return {
    fetch,
    reply: async (data, stale) => {
      await act(async () => {
        pending.shift()?.resolve({ data, stale });
        await Promise.resolve();
      });
    },
    fail: async (message) => {
      await act(async () => {
        pending.shift()?.reject(new Error(message));
        await Promise.resolve();
      });
    },
  };
}

it('shows a snapshot at once, refreshes it fresh in the background and keeps it on failure', async () => {
  const { result } = renderHook(() => useResources());
  const read = controlled();
  act(() => {
    result.current.load('k', read.fetch);
  });
  expect(result.current.get('k')).toEqual({ status: 'loading' });
  await read.reply('snapshot', true);
  expect(result.current.get('k')).toEqual({
    status: 'ready',
    data: 'snapshot',
    refreshing: true,
  });
  expect(read.fetch).toHaveBeenLastCalledWith(true);
  await read.reply('live', false);
  expect(result.current.get('k')).toEqual({
    status: 'ready',
    data: 'live',
    refreshing: false,
  });
  act(() => {
    result.current.revalidate('k');
  });
  expect(result.current.get('k')).toMatchObject({ refreshing: true });
  await read.fail('offline');
  expect(result.current.get('k')).toEqual({
    status: 'ready',
    data: 'live',
    refreshing: false,
  });
  expect(read.fetch).toHaveBeenCalledTimes(3);
});

it('revalidation drops failed reads so they load again, and invalidation forgets data', async () => {
  const { result } = renderHook(() => useResources());
  const read = controlled();
  act(() => {
    result.current.load('a', read.fetch);
  });
  await read.fail('denied');
  expect(result.current.get('a')).toEqual({
    status: 'error',
    message: 'denied',
  });
  act(() => {
    result.current.revalidate('');
  });
  expect(result.current.get('a')).toEqual({ status: 'idle' });
  act(() => {
    result.current.load('a', read.fetch);
  });
  await read.reply('ok', false);
  act(() => {
    result.current.invalidate('a');
  });
  expect(result.current.get('a')).toEqual({ status: 'idle' });
});

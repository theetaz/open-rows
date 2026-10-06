// SPDX-License-Identifier: Apache-2.0
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { SpikeSession, withTimeout } from './lifecycle';

vi.mock('./timer', () => ({
  measurePageTimer: () => ({ resolutionMs: 0.1, crossOriginIsolated: false }),
}));

const mocks = vi.hoisted(() => ({ finish: vi.fn(), progress: vi.fn() }));
vi.mock('@openrows/ipc', () => ({
  finishSpike: mocks.finish,
  spikeProgress: mocks.progress,
}));
beforeEach(() => {
  vi.resetAllMocks();
  mocks.finish.mockResolvedValue(undefined);
  mocks.progress.mockResolvedValue(undefined);
});
afterEach(() => vi.useRealTimers());

it.each(['error', 'unhandledrejection'])(
  'forwards global %s and writes one terminal report',
  async (type) => {
    const session = new SpikeSession();
    const event =
      type === 'error'
        ? new ErrorEvent(type, { message: 'script failed' })
        : Object.assign(new Event(type), {
            reason: new Error('script failed'),
          });
    window.dispatchEvent(event);
    const first = session.finish();
    expect(session.finish('later failure')).toBe(first);
    await first;
    expect(mocks.progress).toHaveBeenCalledWith(
      'error',
      expect.stringContaining('script failed'),
    );
    expect(mocks.finish).toHaveBeenCalledOnce();
    expect(mocks.finish).toHaveBeenCalledWith({
      timer: { resolutionMs: 0.1, crossOriginIsolated: false },
      samples: [],
      error: expect.stringContaining('script failed') as unknown,
    });
    expect(session.signal.aborted).toBe(true);
    window.dispatchEvent(event);
    expect(mocks.finish).toHaveBeenCalledOnce();
  },
);

it('reports a progress-channel failure without recursive global failures', async () => {
  mocks.progress.mockRejectedValue(new Error('diagnostics unavailable'));
  const session = new SpikeSession();
  session.progress('worker-created');
  await session.finish();
  expect(mocks.finish).toHaveBeenCalledWith({
    timer: { resolutionMs: 0.1, crossOriginIsolated: false },
    samples: [],
    error: 'Error: diagnostics unavailable',
  });
  expect(mocks.finish).toHaveBeenCalledOnce();
});

it('clears watchdogs for successful and failed operations', async () => {
  vi.useFakeTimers();
  await expect(withTimeout(Promise.resolve(42), 100, 'test')).resolves.toBe(42);
  await expect(
    withTimeout(Promise.reject(new Error('failed')), 100, 'test'),
  ).rejects.toThrow('failed');
  expect(vi.getTimerCount()).toBe(0);
});

it('finishes with the original error when diagnostic IPC never returns', async () => {
  vi.useFakeTimers();
  mocks.progress.mockReturnValue(new Promise(() => {}));
  const session = new SpikeSession();
  session.progress('worker-created');
  const finished = session.finish('worker initialization failed');
  await vi.advanceTimersByTimeAsync(2_000);
  await finished;
  expect(mocks.finish).toHaveBeenCalledWith({
    timer: { resolutionMs: 0.1, crossOriginIsolated: false },
    samples: [],
    error: 'worker initialization failed',
  });
  expect(vi.getTimerCount()).toBe(0);
});

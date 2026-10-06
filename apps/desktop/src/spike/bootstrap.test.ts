// SPDX-License-Identifier: Apache-2.0
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { startIpcSpike } from './bootstrap';

vi.mock('./timer', () => ({
  measurePageTimer: () => ({ resolutionMs: 0.1, crossOriginIsolated: false }),
}));

const mocks = vi.hoisted(() => ({
  start: vi.fn(),
  run: vi.fn(),
  grid: vi.fn(),
  finish: vi.fn(),
  progress: vi.fn(),
}));
vi.mock('@openrows/ipc', () => ({
  startSpike: mocks.start,
  finishSpike: mocks.finish,
  spikeProgress: mocks.progress,
}));
vi.mock('./ipc', () => ({ runIpcSpike: mocks.run }));
vi.mock('./grid', () => ({ runGridSpike: mocks.grid }));
beforeEach(() => {
  vi.resetAllMocks();
  vi.useFakeTimers();
  mocks.finish.mockResolvedValue(undefined);
});
afterEach(() => vi.useRealTimers());

it('does not start workers outside spike mode', async () => {
  mocks.start.mockResolvedValue(false);
  await startIpcSpike();
  expect(mocks.run).not.toHaveBeenCalled();
  expect(mocks.finish).not.toHaveBeenCalled();
});

it('waits for the show-window handshake before starting the spike', async () => {
  let shown: ((enabled: 'ipc') => void) | undefined;
  mocks.start.mockReturnValue(
    new Promise<'ipc'>((resolve) => {
      shown = resolve;
    }),
  );
  mocks.run.mockImplementation(
    async (session: { finish: () => Promise<void> }) => session.finish(),
  );
  const running = startIpcSpike();
  await Promise.resolve();
  expect(mocks.run).not.toHaveBeenCalled();
  shown?.('ipc');
  await running;
  expect(mocks.run).toHaveBeenCalledOnce();
  expect(vi.getTimerCount()).toBe(0);
});

it('writes an error report if the startup handshake never returns', async () => {
  mocks.start.mockReturnValue(new Promise(() => {}));
  const running = startIpcSpike();
  await vi.advanceTimersByTimeAsync(10_000);
  await running;
  expect(mocks.finish).toHaveBeenCalledWith({
    timer: { resolutionMs: 0.1, crossOriginIsolated: false },
    samples: [],
    error: 'Error: Spike startup handshake timed out after 10000 ms',
  });
});

it('routes grid mode to its lazy module after the native handshake', async () => {
  mocks.start.mockResolvedValue('grid');
  mocks.grid.mockImplementation(
    async (session: { finish: () => Promise<void> }) => session.finish(),
  );
  await startIpcSpike();
  expect(mocks.grid).toHaveBeenCalledOnce();
  expect(mocks.run).not.toHaveBeenCalled();
  expect(vi.getTimerCount()).toBe(0);
});

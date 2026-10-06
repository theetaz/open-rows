// SPDX-License-Identifier: Apache-2.0
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { runIpcSpike } from './ipc';

vi.mock('./timer', () => ({
  measurePageTimer: () => ({ resolutionMs: 0.1, crossOriginIsolated: false }),
}));

const mocks = vi.hoisted(() => ({
  create: vi.fn(),
  initialize: vi.fn(),
  decode: vi.fn(),
  stop: vi.fn(),
  result: vi.fn(),
  finish: vi.fn(),
  progress: vi.fn(),
}));
vi.mock('@openrows/ipc', () => ({
  resultWindow: mocks.result,
  finishSpike: mocks.finish,
  spikeProgress: mocks.progress,
}));
vi.mock('../results/workerClient', () => ({
  ArrowWorker: class {
    constructor() {
      mocks.create();
    }
    initialize = mocks.initialize;
    decode = mocks.decode;
    stop = mocks.stop;
  },
}));
beforeEach(() => {
  vi.useFakeTimers();
  vi.resetAllMocks();
  mocks.finish.mockResolvedValue(undefined);
  mocks.progress.mockResolvedValue(undefined);
  mocks.result.mockResolvedValue(new ArrayBuffer(8));
  mocks.initialize.mockResolvedValue(undefined);
  mocks.decode.mockResolvedValue({
    tile: { rows: 256, columns: Array(20).fill(0) },
    decodeMs: 0.1,
    transferMs: 0.4,
  });
});
afterEach(() => vi.useRealTimers());

it('reports a stalled worker init and stops the worker', async () => {
  mocks.initialize.mockReturnValue(new Promise(() => {}));
  const running = runIpcSpike();
  await vi.advanceTimersByTimeAsync(10_000);
  await running;
  expect(mocks.finish).toHaveBeenCalledWith({
    timer: { resolutionMs: 0.1, crossOriginIsolated: false },
    samples: [],
    error: 'Error: Worker initialization timed out after 10000 ms',
  });
  expect(mocks.stop).toHaveBeenCalledOnce();
});

it.each(['result', 'decode'] as const)(
  'reports a stalled %s tile',
  async (stage) => {
    mocks[stage].mockReturnValue(new Promise(() => {}));
    const running = runIpcSpike();
    await vi.advanceTimersByTimeAsync(2_000);
    await running;
    expect(mocks.finish).toHaveBeenCalledWith({
      timer: { resolutionMs: 0.1, crossOriginIsolated: false },
      samples: [],
      error: expect.stringContaining('timed out after 2000 ms') as unknown,
    });
    expect(mocks.stop).toHaveBeenCalledOnce();
  },
);

it('reports worker construction failures', async () => {
  mocks.create.mockImplementation(() => {
    throw new Error('worker blocked');
  });
  await runIpcSpike();
  expect(mocks.finish).toHaveBeenCalledWith({
    timer: { resolutionMs: 0.1, crossOriginIsolated: false },
    samples: [],
    error: 'Error: worker blocked',
  });
});

it('reports all milestones and 500 samples without leaking timers', async () => {
  await runIpcSpike();
  expect(mocks.finish).toHaveBeenCalledWith({
    timer: { resolutionMs: 0.1, crossOriginIsolated: false },
    samples: expect.any(Array) as unknown,
    error: null,
  });
  const report = mocks.finish.mock.calls[0]?.[0] as { samples: unknown[] };
  expect(report.samples).toHaveLength(500);
  expect(report.samples[0]).toMatchObject({
    decodeMs: 0.4,
    workerDecodeMs: 0.1,
  });
  const stages = mocks.progress.mock.calls.map(([stage]) => stage as unknown);
  expect(stages).toEqual([
    'timer-resolution',
    'worker-created',
    'worker-initialized',
    'first-tile-requested',
    'first-bytes-received',
    'first-decode-done',
    'samples',
    'samples',
    'samples',
    'samples',
    'samples',
    'finishing',
  ]);
  expect(vi.getTimerCount()).toBe(0);
});

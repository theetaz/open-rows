import { paintMetrics } from '@openrows/grid';
// SPDX-License-Identifier: Apache-2.0
import { afterEach, expect, it, vi } from 'vitest';
import type { GridHandle, GridDataSource } from '@openrows/grid';
import { sampleGrid } from './gridSampler';
const cell = { text: 'synthetic', state: 'normal' } as const;
function environment(ignoreScroll = false): {
  handle: GridHandle;
  source: GridDataSource;
  tick: (now: number) => void;
  pending: Map<number, FrameRequestCallback>;
} {
  const pending = new Map<number, FrameRequestCallback>();
  let id = 0;
  vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
    pending.set(++id, callback);
    return id;
  });
  vi.stubGlobal('cancelAnimationFrame', (key: number) => pending.delete(key));
  const viewport = { x: 0, y: 0, width: 1040, height: 600 };
  const metrics = paintMetrics();
  const handle: GridHandle = {
    viewport,
    metrics,
    destroy: () => {},
    scrollTo: (x, y) => {
      if (!ignoreScroll) {
        viewport.x = x;
        viewport.y = y;
      }
      metrics.milliseconds = 1;
      metrics.paints++;
    },
  };
  const source = dataSource();
  const tick = (now: number): void => {
    const entry = pending.entries().next().value;
    if (!entry) throw new Error('No scheduled frame');
    pending.delete(entry[0]);
    entry[1](now);
  };
  return { handle, source, tick, pending };
}
afterEach(() => vi.unstubAllGlobals());
it('retains long frames and preview frames, and reports observed endpoints and paint totals', async () => {
  const context = environment();
  const running = sampleGrid({
    ...context,
    option: 'own',
    scenario: 'jump',
    signal: new AbortController().signal,
  });
  for (const now of [1, 17, 57, 2002, 2018]) context.tick(now);
  const report = await running;
  expect(report.frameMs).toEqual([16, 40, 1945, 16]);
  expect(report.paintMs).toEqual([1, 1, 1, 1]);
  expect(report.endRow).toBe(999_000);
  expect(report.durationMs).toBe(2017);
  expect(context.pending.size).toBe(0);
});
it('does not claim the commanded endpoint when native scrolling was clamped', async () => {
  const context = environment(true);
  const running = sampleGrid({
    ...context,
    option: 'own',
    scenario: 'jump',
    signal: new AbortController().signal,
  });
  const rejected = expect(running).rejects.toThrow('did not settle');
  for (const now of [1, 2002, 3002, 4002]) context.tick(now);
  await rejected;
});
it('waits for placeholder-free paint and fails after the two-second settle timeout', async () => {
  const context = environment();
  const running = sampleGrid({
    ...context,
    option: 'own',
    scenario: 'jump',
    signal: new AbortController().signal,
  });
  const rejected = expect(running).rejects.toThrow('did not settle');
  context.tick(1);
  context.tick(2002);
  for (const now of [2018, 3002, 4002]) {
    context.handle.metrics.placeholders = 1;
    context.tick(now);
  }
  await rejected;
  expect(context.pending.size).toBe(0);
});
it('rejects a placeholder-free frame that arrives after the settle deadline', async () => {
  const context = environment();
  const running = sampleGrid({
    ...context,
    option: 'own',
    scenario: 'jump',
    signal: new AbortController().signal,
  });
  const rejected = expect(running).rejects.toThrow('did not settle');
  for (const now of [1, 2002, 4002]) context.tick(now);
  await rejected;
});
it('cancels the scheduled frame and rejects promptly on a terminal spike failure', async () => {
  const context = environment(),
    abort = new AbortController();
  const running = sampleGrid({
    ...context,
    option: 'own',
    scenario: 'smooth',
    signal: abort.signal,
  });
  const rejected = expect(running).rejects.toThrow('aborted');
  abort.abort();
  await rejected;
  expect(context.pending.size).toBe(0);
});

function dataSource(): GridDataSource {
  return {
    rows: 1_000_000,
    columns: Array.from({ length: 20 }, (_, index) => ({
      id: String(index),
      title: String(index),
      width: 120,
      pinned: index < 2,
    })),
    metrics: {
      requested: 0,
      hits: 0,
      tileApplyMs: 0,
      tileConvertMs: 0,
      workerConvertMs: 0,
      tileAllocations: 0,
      arrayAllocations: 0,
      cellAllocations: 0,
    },
    cell: () => cell,
    window: () => Promise.resolve(),
    subscribe: () => () => {},
  };
}

it('associates a long interval with the preceding command and captures diagnostic counters before resetting', async () => {
  const context = environment();
  const running = sampleGrid({
    ...context,
    option: 'own',
    scenario: 'jump',
    signal: new AbortController().signal,
  });
  context.tick(1);
  context.handle.metrics.scrollHandlerMs = 3;
  context.handle.metrics.canvasResizeEvents = 2;
  context.source.metrics.tileApplyMs = 4;
  context.source.metrics.workerConvertMs = 7;
  context.source.metrics.tileAllocations = 1;
  context.source.metrics.arrayAllocations = 2;
  context.tick(2002);
  expect(context.handle.metrics.scrollHandlerMs).toBe(0);
  expect(context.source.metrics.tileApplyMs).toBe(0);
  context.tick(2042);
  const report = await running;
  expect(report.frameBreakdown[0]).toMatchObject({
    index: 0,
    frameMs: 2001,
    step: 'preview',
    scrollHandlerMs: 3,
    canvasResizeEvents: 2,
    tileApplyMs: 4,
    workerConvertMs: 7,
    tileAllocations: 1,
    arrayAllocations: 2,
  });
  expect(report.frameBreakdown[1]).toMatchObject({
    frameMs: 40,
    step: 'jump row 999000',
    scrollHandlerMs: 0,
    tileApplyMs: 0,
  });
  expect(report.longestFrames).toEqual(report.frameBreakdown);
});

it('prefetches a known destination equally without waiting inside any animation frame', async () => {
  const context = environment();
  const window = vi.fn(() => new Promise<void>(() => {}));
  context.source.window = window;
  const write = vi.spyOn(context.handle, 'scrollTo');
  const running = sampleGrid({
    ...context,
    option: 'own',
    scenario: 'jump',
    signal: new AbortController().signal,
  });
  for (const now of [1, 1901, 1921, 2002, 2018]) context.tick(now);
  await running;
  expect(window).toHaveBeenCalledTimes(1);
  expect(window).toHaveBeenCalledWith(
    expect.objectContaining({ r0: 998_999 }),
    1,
  );
  expect(write).toHaveBeenCalledTimes(4);
  expect(context.pending.size).toBe(0);
});

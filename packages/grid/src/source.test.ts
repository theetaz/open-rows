// SPDX-License-Identifier: Apache-2.0
import { expect, it, vi } from 'vitest';
import { TileSource } from './source';
import { props, tile } from './testFixture';
const range = { r0: 512, r1: 535, c0: 40, c1: 49 };
it('deduplicates windows, fills before notifying, and prefetches in both directions', async () => {
  const load = vi.fn(tile),
    source = new TileSource('spike', 1_000_000, props().source.columns, load);
  const listener = vi.fn(() => {
    expect(source.cell(512, 40)).toBeDefined();
  });
  await Promise.all([source.window(range, 1), source.window(range, 1)]);
  expect(
    load.mock.calls.map(([request]) => [request.rowStart, request.colStart]),
  ).toEqual([
    [512, 0],
    [512, 40],
    [768, 0],
    [768, 40],
    [512, 60],
  ]);
  const unsubscribe = source.subscribe(listener);
  await source.window({ ...range, r0: 1024, r1: 1047 }, -1);
  expect(load.mock.calls.some(([request]) => request.rowStart === 768)).toBe(
    true,
  );
  expect(listener).toHaveBeenCalled();
  unsubscribe();
  await source.window(range, 0);
  expect(source.metrics.hits).toBeGreaterThan(0);
  expect(source.cell(0, 0)).toBeUndefined();
});
it('keeps only 64 display tiles using the S1 LRU, including pinned requests', async () => {
  const load = vi.fn(tile),
    source = new TileSource('spike', 1_000_000, props().source.columns, load);
  for (let index = 0; index < 65; index++)
    await source.window(
      { r0: index * 256, r1: index * 256 + 1, c0: 0, c1: 1 },
      0,
    );
  expect(source.cell(1, 0)).toBeUndefined();
  expect(source.cell(64 * 256 + 1, 0)?.text).toBe(String(64 * 256 + 1));
  await source.window({ r0: 0, r1: 1, c0: 0, c1: 1 }, 0);
  expect(load).toHaveBeenCalledTimes(66);
});
it('converts NULL and visual states once, and clamps the last tile', async () => {
  const source = props().source;
  await source.window({ r0: 0, r1: 10, c0: 0, c1: 19 }, 0);
  expect(source.cell(0, 0)).toEqual({ text: 'NULL', state: 'null' });
  expect(source.cell(2, 2)?.state).toBe('changed');
  expect(source.cell(4, 0)?.state).toBe('deleted');
  await source.window({ r0: 999_999, r1: 999_999, c0: 99, c1: 99 }, 1);
  expect(source.cell(999_999, 99)?.text).toBe('999999');
});

it('accepts worker-ready cells without converting Arrow on the main thread and deduplicates intratile scroll windows', async () => {
  const cells = Array.from({ length: 5120 }, () => ({
    text: 'exact',
    state: 'normal' as const,
  }));
  const load = vi.fn(() =>
    Promise.resolve({ cells, columns: 20, convertMs: 7 }),
  );
  const source = new TileSource(
    'spike',
    1_000_000,
    props().source.columns,
    load,
  );
  const first = source.window({ r0: 0, r1: 23, c0: 2, c1: 8 }, 1);
  expect(source.window({ r0: 1, r1: 24, c0: 3, c1: 9 }, 1)).toBe(first);
  await first;
  expect(source.cell(1, 2)).toBe(cells[22]);
  expect(source.metrics.workerConvertMs).toBe(21);
  expect(source.metrics.tileAllocations).toBe(3);
  expect(source.metrics.cellAllocations).toBe(15_360);
});
it('grows a streaming result and refetches only the partial last tile row', async () => {
  const load = vi.fn(tile),
    columns = props().source.columns.slice(0, 3),
    source = new TileSource('spike', 300, columns, load);
  const window = { r0: 0, r1: 299, c0: 0, c1: 2 };
  await source.window(window, 0);
  expect(source.cell(299, 0)).toBeDefined();
  expect(source.cell(300, 0)).toBeUndefined();
  const listener = vi.fn();
  source.subscribe(listener);
  source.grow(200);
  expect(listener).not.toHaveBeenCalled();
  source.grow(600);
  expect(source.rows).toBe(600);
  expect(listener).toHaveBeenCalledOnce();
  // The full first tile row stays cached; the cut one is gone until refetched.
  expect(source.cell(0, 0)).toBeDefined();
  expect(source.cell(299, 0)).toBeUndefined();
  load.mockClear();
  await source.window({ ...window, r1: 599 }, 0);
  expect(load.mock.calls.map(([request]) => request.rowStart)).toEqual([
    256, 512,
  ]);
  expect(source.cell(511, 0)).toBeDefined();
});

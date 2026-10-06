// SPDX-License-Identifier: Apache-2.0
import { expect, it, vi } from 'vitest';
import { paintMetrics } from '@openrows/grid';
import { observeReactCommits } from './reactTiming';
const timing = globalThis as typeof globalThis & {
  __openrowsGridTiming?: { begin(): number; end(started: number): void };
};
it('attributes complete commits to the active scenario and removes its observer', () => {
  const metrics = paintMetrics();
  const clock = vi
    .spyOn(performance, 'now')
    .mockReturnValueOnce(10)
    .mockReturnValueOnce(26);
  const stop = observeReactCommits(metrics);
  try {
    const observer = timing.__openrowsGridTiming;
    if (!observer) throw new Error('Commit observer missing');
    observer.end(observer.begin());
    expect(metrics.reactCommitMs).toBe(16);
  } finally {
    stop();
    clock.mockRestore();
  }
  expect(timing.__openrowsGridTiming).toBeUndefined();
});
it('old cleanup cannot remove a newer scenario observer', () => {
  const first = observeReactCommits(paintMetrics());
  const second = observeReactCommits(paintMetrics());
  first();
  expect(timing.__openrowsGridTiming).toBeDefined();
  second();
  second();
  expect(timing.__openrowsGridTiming).toBeUndefined();
});

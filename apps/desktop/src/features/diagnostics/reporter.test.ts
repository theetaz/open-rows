// SPDX-License-Identifier: Apache-2.0
import { expect, it, vi } from 'vitest';
import type { UiReport } from '@openrows/ipc';
import { createReporter, rateLimiter } from './reporter';

const report: UiReport = {
  kind: 'error',
  name: 'TypeError',
  message: 'boom',
  durationMs: null,
};

it('admits at most the limit per window and again once the window passes', () => {
  let time = 0;
  const admit = rateLimiter(2, 1_000, () => time);
  expect([admit(), admit(), admit()]).toEqual([true, true, false]);
  time = 999;
  expect(admit()).toBe(false);
  time = 1_000;
  expect(admit()).toBe(true);
});

it('sends only admitted reports, and a failed send never escapes', async () => {
  const send = vi.fn(() => Promise.reject(new Error('offline')));
  const sendOnce = createReporter(send, rateLimiter(1, 60_000));
  expect(() => {
    sendOnce(report);
    sendOnce(report);
  }).not.toThrow();
  await Promise.resolve();
  expect(send).toHaveBeenCalledTimes(1);
});

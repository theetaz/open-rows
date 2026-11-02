// SPDX-License-Identifier: Apache-2.0
import { afterEach, expect, it, vi } from 'vitest';
import { GcProbe } from './gcProbe';

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

it('reports the three longest main-thread timer gaps relative to start', () => {
  vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
  vi.stubGlobal('requestAnimationFrame', () => 1);
  vi.stubGlobal('cancelAnimationFrame', () => undefined);
  const clock = { now: 1_000 };
  vi.spyOn(performance, 'now').mockImplementation(() => clock.now);
  const probe = new GcProbe();
  probe.start();
  for (const step of [4, 4, 150, 4, 30, 4, 60, 4]) {
    clock.now += step;
    vi.advanceTimersByTime(4);
  }
  probe.stop();
  expect(probe.summary()).toBe('gap 150@8 gap 60@196 gap 30@162');
  clock.now += 500;
  vi.advanceTimersByTime(500);
  expect(probe.summary()).toBe('gap 150@8 gap 60@196 gap 30@162');
  vi.unstubAllGlobals();
});

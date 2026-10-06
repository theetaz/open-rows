// SPDX-License-Identifier: Apache-2.0
import { afterEach, expect, it, vi } from 'vitest';
import { measurePageTimer } from './timer';

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

it('records the minimum nonzero consecutive delta and actual isolation', () => {
  vi.stubGlobal('crossOriginIsolated', true);
  vi.spyOn(performance, 'now')
    .mockReturnValueOnce(0)
    .mockReturnValueOnce(0)
    .mockReturnValueOnce(0.5)
    .mockReturnValueOnce(0.625)
    .mockReturnValue(1);
  expect(measurePageTimer()).toEqual({
    resolutionMs: 0.125,
    crossOriginIsolated: true,
  });
});

it('reports unavailable resolution rather than inventing precision for a frozen clock', () => {
  vi.stubGlobal('crossOriginIsolated', false);
  vi.spyOn(performance, 'now').mockReturnValue(10);
  expect(measurePageTimer()).toEqual({
    resolutionMs: null,
    crossOriginIsolated: false,
  });
});

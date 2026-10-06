// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { scenarioDuration, scrollTarget, SCENARIOS } from './gridScenarios';
it('samples top, middle and near bottom at 6000 px/s for ten seconds total', () => {
  const target = { x: 0, y: 0 };
  for (const [time, start] of [
    [0, 0],
    [10_000 / 3, 13_000_000],
    [20_000 / 3, 25_979_426],
  ] as const) {
    scrollTarget('smooth', time, target);
    expect(target.y).toBeCloseTo(start);
    scrollTarget('smooth', time + 1000, target);
    expect(target.y).toBeCloseTo(start + 6000);
  }
  scrollTarget('smooth', 10_000, target);
  expect(Math.floor(target.y / 26)).toBe(999977);
  expect(
    SCENARIOS.reduce((sum, scenario) => sum + scenarioDuration(scenario), 0),
  ).toBe(38_500);
});
it('commands sixty large jumps covering the range then sixty page-down steps', () => {
  const target = { x: 0, y: 0 };
  const jumps = [];
  for (let step = 0; step < 60; step++) {
    scrollTarget('flings', step * 100, target);
    jumps.push(target.y);
  }
  expect(new Set(jumps).size).toBe(60);
  expect(Math.min(...jumps)).toBe(0);
  expect(Math.max(...jumps)).toBe(999_000 * 26);
  for (let step = 60; step < 120; step++) {
    scrollTarget('flings', step * 100, target);
    expect(target.y).toBe(500_000 * 26 + (step - 59) * 574);
  }
});
it('scrolls horizontally at 3000 px/s at the middle and jumps to the far row', () => {
  const target = { x: 0, y: 0 };
  scrollTarget('horizontal', 1000, target);
  expect(target).toEqual({ x: 3000, y: 500_000 * 26 });
  scrollTarget('horizontal', 5000, target);
  expect(target.x).toBe(10960);
  scrollTarget('jump', 0, target);
  expect(target.y).toBe(999000 * 26);
});

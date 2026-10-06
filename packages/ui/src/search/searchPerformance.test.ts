// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { rankPaletteItems } from './paletteResults';
const catalog = Array.from({ length: 1000 }, (_, index) => ({
  id: String(index),
  label: `orders_${String(index)}_summary`,
  searchText: `orders_${String(index)}_summary reporting`,
}));
it('PERF: ranking 1,000 catalog objects stays within a 16 ms p95 budget', () => {
  const durations: number[] = [];
  for (let round = 0; round < 110; round++) {
    const started = performance.now();
    const results = rankPaletteItems(catalog, 'ord12');
    const duration = performance.now() - started;
    expect(results.length).toBeGreaterThan(0);
    if (round >= 10) durations.push(duration);
  }
  durations.sort((left, right) => left - right);
  const p95 = durations[94] ?? Infinity;
  process.stdout.write(
    `Catalog ranking p95: ${p95.toFixed(3)} ms / 16 ms budget (1,000 objects, 100 samples)\n`,
  );
  expect(p95).toBeLessThanOrEqual(16);
});

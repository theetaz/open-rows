// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { samplePlan } from '../../test/planStub';
import { flameHeat, flameLayout, flameNext } from './flameLayout';

it('uses exclusive actual time for fills and descendant sums for envelopes', () => {
  const plan = samplePlan();
  const cells = flameLayout(plan);
  const total = 7.8;
  expect(cells[0]?.width).toBeCloseTo(1.2 / total);
  expect(cells[0]?.span).toBeCloseTo(1);
  expect(cells[1]?.x).toBeCloseTo(1.2 / total);
  expect(cells[2]?.width).toBeCloseTo(4.3 / total);
  expect(cells[3]?.x).toBeCloseTo(5.9 / total);
  expect(cells[0]?.heat).toBe('high');
  expect(cells[2]?.heat).toBe('high');
  expect(cells[3]?.heat).toBe('medium');
});

it('falls back to exclusive cost, then equal widths when measures are absent', () => {
  const plan = samplePlan({ analyzed: false, weight: 'cost' });
  plan.nodes = plan.nodes.map((node, index) => ({
    ...node,
    exclusiveCost: index + 1,
  }));
  expect(flameLayout(plan).map((cell) => cell.width)).toEqual([
    0.1, 0.2, 0.3, 0.4,
  ]);
  plan.weight = 'none';
  plan.nodes = plan.nodes.map((node) => ({ ...node, exclusiveCost: null }));
  expect(flameLayout(plan).map((cell) => cell.width)).toEqual([
    0.25, 0.25, 0.25, 0.25,
  ]);
  expect(flameLayout({ ...plan, nodes: [] })).toEqual([]);
});

it('colours misestimates/scans as warnings and spills as danger', () => {
  const node = samplePlan().nodes[0];
  if (!node) throw new Error('missing test node');
  expect(
    flameHeat({
      ...node,
      share: 0,
      hotspots: [{ kind: 'misestimate', estimated: 1, actual: 100 }],
    }),
  ).toBe('medium');
  expect(flameHeat({ ...node, share: 0, hotspots: [] })).toBe('none');
});

it('navigates hierarchy and pre-order without moving beyond the plan', () => {
  const { nodes } = samplePlan();
  expect(flameNext('ArrowLeft', nodes, 2)).toBe(1);
  expect(flameNext('ArrowRight', nodes, 1)).toBe(2);
  expect(flameNext('ArrowUp', nodes, 0)).toBe(0);
  expect(flameNext('ArrowDown', nodes, 3)).toBe(3);
  expect(flameNext('Escape', nodes, 0)).toBeUndefined();
});

it('lays out 2000 nodes iteratively; opt-in timing reports layout latency', () => {
  const plan = samplePlan();
  const node = plan.nodes[0];
  if (!node) throw new Error('missing test node');
  plan.nodes = Array.from({ length: 2000 }, (_, id) => ({
    ...node,
    id,
    parent: id === 0 ? null : Math.floor((id - 1) / 2),
    depth: Math.floor(Math.log2(id + 1)),
    exclusiveMs: 1,
  }));
  const cells = flameLayout(plan);
  expect(cells).toHaveLength(2000);
  expect(cells[0]?.span).toBeCloseTo(1);
  expect(cells[1999]?.width).toBeCloseTo(1 / 2000);
  if (process.env.OPENROWS_BENCH === '1') {
    const samples = Array.from({ length: 100 }, () => {
      const start = performance.now();
      flameLayout(plan);
      return performance.now() - start;
    }).sort((a, b) => a - b);
    process.stdout.write(
      `Flame 2000 nodes layout: median=${(samples[50] ?? 0).toFixed(3)} ms p95=${(samples[95] ?? 0).toFixed(3)} ms (16 ms frame reference)\n`,
    );
  }
});

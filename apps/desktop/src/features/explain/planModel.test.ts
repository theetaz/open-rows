// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { samplePlan } from '../../test/planStub';
import {
  formatMs,
  formatNumber,
  formatShare,
  hasChildren,
  heatOf,
  hotspotText,
  hotspotsOf,
  misestimateFactor,
  nodeLabel,
  planSummary,
  rowsText,
  selfText,
  visibleNodes,
} from './planModel';

it('grades heat by share of the plan', () => {
  expect([0, 0.04, 0.05, 0.2, 0.5, null].map(heatOf)).toEqual([
    'none',
    'none',
    'low',
    'medium',
    'high',
    'none',
  ]);
});

it('formats times, numbers and shares compactly', () => {
  expect(formatMs(0.0123)).toBe('0.012 ms');
  expect(formatMs(4.567)).toBe('4.57 ms');
  expect(formatMs(45.67)).toBe('45.7 ms');
  expect(formatMs(1234)).toBe('1.23 s');
  expect(formatMs(null)).toBe('—');
  expect(formatNumber(12345.678)).toBe('12,345.68');
  expect(formatShare(0.004)).toBe('<1%');
  expect(formatShare(0.456)).toBe('46%');
});

it('labels nodes and their measures in the plan weight', () => {
  const plan = samplePlan();
  const [, , scan, lookup] = plan.nodes;
  if (!scan || !lookup) throw new Error('sample plan changed');
  expect(nodeLabel(lookup)).toBe(
    'Index Scan on public.orders using orders_customer',
  );
  expect(selfText(plan, scan)).toBe('4.30 ms');
  expect(rowsText(plan, scan)).toBe('200 / 1');
  const estimated = samplePlan({ analyzed: false, weight: 'cost' });
  expect(rowsText(estimated, scan)).toBe('1');
  expect(selfText(estimated, scan)).toBe('1');
  expect(selfText(samplePlan({ weight: 'none' }), scan)).toBe('—');
});

it('explains each hotspot kind with a tone', () => {
  expect(misestimateFactor(1, 200)).toBe(200);
  expect(misestimateFactor(5000, 3)).toBe(1667);
  expect(
    hotspotText({ kind: 'misestimate', estimated: 1, actual: 200 }),
  ).toMatchObject({
    label: 'Rows ×200',
    tone: 'warning',
  });
  expect(hotspotText({ kind: 'expensive', share: 0.55 }).label).toBe(
    '55% of plan',
  );
  expect(hotspotText({ kind: 'fullScan', rows: null }).detail).toMatch(
    /whole table/,
  );
  expect(hotspotText({ kind: 'spill', reason: 'Hash split' })).toEqual({
    label: 'Disk spill',
    detail: 'Hash split',
    tone: 'danger',
  });
});

it('lists hotspots with the most expensive first', () => {
  const kinds = hotspotsOf(samplePlan()).map(({ hotspot }) => hotspot.kind);
  expect(kinds[0]).toBe('expensive');
  expect(kinds).toHaveLength(4);
});

it('hides the inputs of collapsed operators', () => {
  const { nodes } = samplePlan();
  const [root, loop, scan] = nodes;
  if (!root || !loop || !scan) throw new Error('sample plan changed');
  expect(hasChildren(nodes, loop)).toBe(true);
  expect(hasChildren(nodes, scan)).toBe(false);
  expect(visibleNodes(nodes, new Set([1])).map((node) => node.id)).toEqual([
    0, 1,
  ]);
  expect(visibleNodes(nodes, new Set([0]))).toHaveLength(1);
  expect(visibleNodes(nodes, new Set())).toHaveLength(4);
});

it('summarises the run and the rollback', () => {
  expect(planSummary(samplePlan())).toEqual([
    'Explain Analyze',
    'Execution 7.80 ms',
    'Planning 0.200 ms',
    '4 operators',
    'Changes rolled back',
  ]);
  const plain = samplePlan({
    analyzed: false,
    rolledBack: false,
    executionMs: null,
    planningMs: null,
  });
  expect(planSummary(plain)).toEqual(['Explain', '4 operators']);
});

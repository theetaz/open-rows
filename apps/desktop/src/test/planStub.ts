// SPDX-License-Identifier: Apache-2.0
import type { PlanDto, PlanNodeDto } from '@openrows/ipc';

/** A plan operator with neutral values; every field can be overridden. */
function planNode(
  id: number,
  overrides: Partial<PlanNodeDto> = {},
): PlanNodeDto {
  return {
    id,
    parent: id === 0 ? null : 0,
    depth: id === 0 ? 0 : 1,
    nodeType: 'Result',
    relation: null,
    index: null,
    estimatedRows: 1,
    actualRows: 1,
    loops: 1,
    startupCost: 0,
    totalCost: 1,
    actualStartupMs: 0,
    actualTotalMs: 1,
    inclusiveMs: 1,
    exclusiveMs: 1,
    inclusiveCost: 1,
    exclusiveCost: 1,
    share: 0,
    buffers: null,
    conditions: [],
    details: [],
    hotspots: [],
    ...overrides,
  };
}

/**
 * An analysed join shaped like real PostgreSQL output: a spilling sort over
 * a nested loop of a misestimated full scan and an index scan.
 */
export function samplePlan(overrides: Partial<PlanDto> = {}): PlanDto {
  return {
    statement:
      'SELECT * FROM orders o JOIN customers c ON c.id = o.customer_id',
    analyzed: true,
    rolledBack: true,
    weight: 'time',
    planningMs: 0.2,
    executionMs: 7.8,
    summary: [],
    nodes: [
      planNode(0, {
        nodeType: 'Sort',
        share: 0.15,
        exclusiveMs: 1.2,
        hotspots: [
          {
            kind: 'spill',
            reason: 'Sort spilled to disk (external merge, 1832 kB)',
          },
        ],
      }),
      planNode(1, { nodeType: 'Nested Loop', share: 0.05, exclusiveMs: 0.4 }),
      planNode(2, {
        parent: 1,
        depth: 2,
        nodeType: 'Seq Scan',
        relation: 'public.customers',
        estimatedRows: 1,
        actualRows: 200,
        share: 0.55,
        exclusiveMs: 4.3,
        conditions: [{ label: 'Filter', value: "(region = 'eu')" }],
        hotspots: [
          { kind: 'expensive', share: 0.55 },
          { kind: 'misestimate', estimated: 1, actual: 200 },
          { kind: 'fullScan', rows: 20000 },
        ],
      }),
      planNode(3, {
        parent: 1,
        depth: 2,
        nodeType: 'Index Scan',
        relation: 'public.orders',
        index: 'orders_customer',
        loops: 200,
        share: 0.25,
        exclusiveMs: 1.9,
        buffers: {
          sharedHit: 600,
          sharedRead: 2,
          sharedDirtied: 0,
          sharedWritten: 0,
          tempRead: 0,
          tempWritten: 0,
        },
      }),
    ],
    raw: '[{"Plan": {"Node Type": "Sort"}}]',
    rawFormat: 'json',
    ...overrides,
  };
}

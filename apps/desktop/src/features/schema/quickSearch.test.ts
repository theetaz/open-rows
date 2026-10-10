// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { quickIndex, searchQuick } from './quickSearch';
it('ranks objects, local queries and project paths with NameIndex ordering', () => {
  const index = quickIndex([
    { id: 'object', label: 'orders', searchText: 'orders public.orders' },
    { id: 'saved', label: 'orders saved' },
    {
      id: 'project',
      label: 'recent_orders',
      searchText: 'recent_orders queries/recent_orders.sql',
    },
  ]);
  expect(searchQuick(index, 'orders').map((item) => item.id)).toEqual([
    'object',
    'saved',
    'project',
  ]);
  expect(searchQuick(index, 'rcnt').map((item) => item.id)).toEqual([
    'project',
  ]);
  expect(searchQuick(index, 'queries/recent').map((item) => item.id)).toEqual([
    'project',
  ]);
});
it('cached quick-open p95 stays below 50ms at 10k objects plus 1k queries', () => {
  const objects = Array.from({ length: 10_000 }, (_, index) => ({
    id: `object-${String(index)}`,
    label: `orders_${String(index)}`,
    searchText: `public.orders_${String(index)}`,
  }));
  const queries = Array.from({ length: 1_000 }, (_, index) => ({
    id: `query-${String(index)}`,
    label: `query_${String(index)}`,
    searchText: `queries/query_${String(index)}.sql`,
  }));
  const index = quickIndex([...objects, ...queries]);
  const samples = Array.from({ length: 100 }, (_, attempt) => {
    const start = performance.now();
    searchQuick(
      index,
      ['orders', 'query_99', 'qrs', 'public.orders_1'][attempt % 4] ?? '',
      100,
    );
    return performance.now() - start;
  }).sort((left, right) => left - right);
  const p95 = samples[94] ?? Infinity;
  expect(
    p95,
    `cached quick-open p95: ${p95.toFixed(3)} ms (11000 entries)`,
  ).toBeLessThanOrEqual(50);
});

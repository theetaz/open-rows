// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import {
  arity,
  countKey,
  filterRow,
  initialView,
  nextSort,
  pageKey,
  rangeLabel,
  tableQuery,
  toFilters,
} from './model';

const target = {
  profileId: 'p',
  database: 'orders',
  schema: 'public',
  table: 'users',
};

it('cycles header sort ascending, descending, none', () => {
  const ascending = nextSort(null, 'id');
  expect(ascending).toEqual({ column: 'id', descending: false });
  const descending = nextSort(ascending, 'id');
  expect(descending).toEqual({ column: 'id', descending: true });
  expect(nextSort(descending, 'id')).toBeNull();
  expect(nextSort(descending, 'name')).toEqual({
    column: 'name',
    descending: false,
  });
});

it('applies enabled rows, or a single row', () => {
  const rows = [
    { ...filterRow(1, 'id'), value: '1' },
    { ...filterRow(2, 'name'), enabled: false, operator: 'isNull' as const },
  ];
  expect(toFilters(rows)).toEqual([
    { column: 'id', operator: 'equal', value: '1', second: '' },
  ]);
  expect(toFilters(rows, 2)).toEqual([
    { column: 'name', operator: 'isNull', value: '', second: '' },
  ]);
  expect([arity('isNull'), arity('in'), arity('between')]).toEqual([0, 1, 2]);
});

it('projects only visible columns once some are hidden', () => {
  expect(tableQuery(target, initialView, ['id', 'name']).columns).toEqual([]);
  const hidden = { ...initialView, hidden: ['name'] };
  expect(tableQuery(target, hidden, ['id', 'name', 'email']).columns).toEqual([
    'id',
    'email',
  ]);
});

it('keys pages by request and counts by filters only', () => {
  const query = tableQuery(target, initialView, []);
  const next = { ...initialView, offset: 300 };
  expect(pageKey(query, initialView, 's')).not.toBe(pageKey(query, next, 's'));
  const sorted = { ...query, sort: { column: 'id', descending: true } };
  expect(countKey(sorted)).toBe(countKey(query));
  const filtered = {
    ...query,
    filters: toFilters([filterRow(1, 'id')]),
  };
  expect(countKey(filtered)).not.toBe(countKey(query));
});

it('labels the shown range against exact, estimated or unknown totals', () => {
  const none = { exact: null, estimate: null };
  expect(rangeLabel(0, 300, { exact: null, estimate: 1811 })).toBe(
    '1–300 of ~1,811 rows',
  );
  expect(rangeLabel(300, 300, { exact: 1811, estimate: 1811 })).toBe(
    '301–600 of 1,811 rows',
  );
  expect(rangeLabel(1800, 11, { exact: null, estimate: 100 })).toBe(
    '1,801–1,811 of ~1,811 rows',
  );
  expect(rangeLabel(0, 12, none)).toBe('1–12 rows');
  expect(rangeLabel(0, 0, none)).toBe('No rows');
  expect(rangeLabel(300, 0, none)).toBe('No rows after 300');
});

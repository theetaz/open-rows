// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import {
  defaultSchema,
  formatEstimate,
  objectLabel,
  objectRef,
  sameObject,
} from './catalog';
import { closeIn } from './useNavigation';

it('formats planner estimates compactly and marks them approximate', () => {
  expect(formatEstimate(null)).toBeUndefined();
  expect(formatEstimate(0)).toBe('~0');
  expect(formatEstimate(999)).toBe('~999');
  expect(formatEstimate(412_000)).toBe('~412K');
  expect(formatEstimate(2_400_000)).toBe('~2.4M');
  expect(formatEstimate(3_000_000_000)).toBe('~3B');
});

it('prefers public, then the first user schema', () => {
  const schema = (name: string, system = false) => ({
    name,
    system,
    comment: null,
  });
  expect(defaultSchema([schema('a'), schema('public')])).toBe('public');
  expect(defaultSchema([schema('pg_catalog', true), schema('b')])).toBe('b');
  expect(defaultSchema([schema('pg_catalog', true)])).toBe('pg_catalog');
  expect(defaultSchema([])).toBe('');
});

it('labels routines with their arguments and tells overloads apart', () => {
  const tax = {
    schema: 'public',
    name: 'tax',
    kind: 'function' as const,
    arguments: 'numeric',
    rowEstimate: null,
    comment: null,
  };
  expect(objectLabel(tax)).toBe('tax(numeric)');
  expect(objectLabel({ name: 'orders', arguments: null })).toBe('orders');
  const one = objectRef('db', tax);
  expect(sameObject(one, { ...one })).toBe(true);
  expect(sameObject(one, { ...one, arguments: 'integer' })).toBe(false);
});

it('keeps the last opened database and moves off a closed active one', () => {
  const nav = {
    active: 'a',
    opened: ['a', 'b'],
    schemas: {},
    systemSchemas: false,
  };
  expect(closeIn(nav, 'a')).toMatchObject({ active: 'b', opened: ['b'] });
  const single = { ...nav, opened: ['a'] };
  expect(closeIn(single, 'a')).toBe(single);
});

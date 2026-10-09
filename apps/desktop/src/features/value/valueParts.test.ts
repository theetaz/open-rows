// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { arrayItems, valueParts } from './valueParts';

it('splits one-dimensional array literals into unquoted items', () => {
  expect(arrayItems('{1,NULL,3}')).toEqual(['1', null, '3']);
  expect(arrayItems('{"a b","",x,"q\\"\\\\"}')).toEqual([
    'a b',
    '',
    'x',
    'q"\\',
  ]);
  expect(arrayItems('{}')).toEqual([]);
  expect(arrayItems('{{1,2},{3,4}}')).toBeNull();
  expect(arrayItems('[0:1]={1,2}')).toBeNull();
});

it('reads range bounds, emptiness and inclusivity', () => {
  expect(valueParts('int4range', '[1,10)')).toEqual({
    kind: 'range',
    empty: false,
    lower: '1',
    upper: '10',
    lowerInclusive: true,
    upperInclusive: false,
  });
  expect(valueParts('tsrange', '(,"2024-01-01 00:00:00"]')).toMatchObject({
    lower: null,
    upper: '2024-01-01 00:00:00',
    upperInclusive: true,
  });
  expect(valueParts('daterange', 'empty')).toMatchObject({ empty: true });
  expect(valueParts('datemultirange', '{[2024-01-01,2024-02-01)}')).toBeNull();
});

it('splits network addresses and ignores other types', () => {
  expect(valueParts('cidr', '10.1.0.0/16')).toEqual({
    kind: 'network',
    address: '10.1.0.0',
    prefix: '16',
  });
  expect(valueParts('inet', '::1')).toMatchObject({ prefix: null });
  expect(valueParts('integer[]', '{1}')).toEqual({
    kind: 'array',
    items: ['1'],
  });
  expect(valueParts('text', '{1}')).toBeNull();
  expect(valueParts(null, '{1}')).toBeNull();
});

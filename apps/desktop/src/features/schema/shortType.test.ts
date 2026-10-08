// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { shortType } from './shortType';

it('uses PostgreSQL short type names like TablePlus', () => {
  expect(shortType('double precision')).toBe('float8');
  expect(shortType('character varying(255)')).toBe('varchar');
  expect(shortType('character(2)')).toBe('bpchar');
  expect(shortType('timestamp without time zone')).toBe('timestamp');
  expect(shortType('timestamp(6) with time zone')).toBe('timestamptz');
  expect(shortType('integer[]')).toBe('int4[]');
  expect(shortType('numeric(10,2)')).toBe('numeric');
  expect(shortType('jsonb')).toBe('jsonb');
  expect(shortType('public.mood')).toBe('public.mood');
});

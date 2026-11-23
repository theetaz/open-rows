// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { columnAlign } from './align';

it.each([
  'integer',
  'bigint',
  'int4',
  'numeric(10,2)',
  'double precision',
  'money',
  'bigserial',
  'tinyint(1) unsigned',
  'MEDIUMINT',
  'decimal(12,4)',
  'double',
  'smallmoney',
  'UInt64',
  'Int8',
  'Float64',
  'Decimal128(4)',
  'Nullable(Int32)',
  'HUGEINT',
  'REAL',
])('right-aligns numeric type %s', (type) => {
  expect(columnAlign(type)).toBe('end');
});

it.each([
  null,
  'text',
  'varchar(20)',
  'interval',
  'timestamp with time zone',
  'point',
  'Nullable(String)',
  'internal',
  'json',
])('keeps %s start-aligned', (type) => {
  expect(columnAlign(type)).toBe('start');
});

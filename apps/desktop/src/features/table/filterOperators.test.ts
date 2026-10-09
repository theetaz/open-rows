// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { operatorsFor } from './filterOperators';
import { columnPatch } from './FilterRowEditor';
import { filterRow } from './model';

const values = (type: string | null, engine = 'postgres' as const) =>
  operatorsFor(type, engine).map((item) => item.value);

it('offers equality for booleans, ranges for numbers and dates, patterns for text', () => {
  expect(values('bool')).toEqual([
    'equal',
    'notEqual',
    'in',
    'isNull',
    'isNotNull',
    'notIn',
  ]);
  for (const type of ['int4', 'numeric(10,2)', 'timestamptz', 'date'])
    expect(values(type), type).toEqual(
      expect.arrayContaining(['less', 'between', 'notBetween', 'in']),
    );
  expect(values('int4')).not.toContain('contains');
  expect(values('text')).toEqual(
    expect.arrayContaining([
      'contains',
      'startsWith',
      'like',
      'regex',
      'isEmpty',
    ]),
  );
  expect(values(null)).toEqual(values('varchar'));
  for (const type of ['bool', 'int4', 'text', null])
    expect(values(type), String(type)).not.toContain('raw');
});

it('offers regex only where the engine has it', () => {
  expect(values('text', 'sqlite' as never)).not.toContain('regex');
  expect(values('text', 'sqlite' as never)).toContain('like');
  expect(operatorsFor('text', 'mysql').map((item) => item.value)).toContain(
    'notRegex',
  );
});

it('switches a row to Raw SQL and back, keeping operators the column allows', () => {
  const row = { ...filterRow(1, 'name'), operator: 'contains' as const };
  expect(columnPatch(row, '', [])).toEqual({ column: '', operator: 'raw' });
  expect(columnPatch(row, 'title', operatorsFor('text', 'postgres'))).toEqual({
    column: 'title',
    operator: 'contains',
  });
  expect(columnPatch(row, 'id', operatorsFor('int4', 'postgres'))).toEqual({
    column: 'id',
    operator: 'equal',
  });
});

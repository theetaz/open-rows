// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import type { TableColumnDto } from '@openrows/ipc';
import { DEFAULT_VALUE, NULL_VALUE, textValue } from '../cellValue';
import { cellError, summarize, typeError } from './validate';

function column(
  dataType: string,
  extra: Partial<TableColumnDto> = {},
): TableColumnDto {
  return {
    name: 'amount',
    dataType,
    nullable: false,
    default: null,
    comment: null,
    primaryKey: false,
    enumLabels: null,
    ...extra,
  };
}

it.each([
  ['integer', '42', null],
  ['integer', ' -7 ', null],
  ['bigint', '4.5', 'is not an integer'],
  ['integer', 'abc', 'is not an integer'],
  ['numeric(10,2)', '12.50', null],
  ['double precision', '1e-3', null],
  ['real', 'NaN', null],
  ['numeric', 'twelve', 'is not a number'],
  ['boolean', 'yes', null],
  ['bool', 'maybe', 'is not a boolean'],
  ['uuid', '123e4567-e89b-12d3-a456-426614174000', null],
  ['uuid', 'not-a-uuid', 'is not a UUID'],
  ['jsonb', '{"a":[1,2]}', null],
  ['json', '{a:1}', 'is not valid JSON'],
  ['text', 'anything', null],
  ['timestamp with time zone', 'yesterday', null],
  ['integer[]', '{1,2}', null],
  ['public.mood', 'happy', null],
])('checks %s input %j → %s', (dataType, text, expected) => {
  expect(typeError(dataType, text)).toBe(expected);
});

it('refuses locked cells, NULL on NOT NULL columns, DEFAULT without a default, and bad text', () => {
  expect(cellError(column('integer'), true, textValue('1'))).toBe(
    'This field cannot be edited.',
  );
  expect(cellError(undefined, false, textValue('1'))).toBe(
    'This field cannot be edited.',
  );
  expect(cellError(column('integer'), false, NULL_VALUE)).toBe(
    'amount does not accept NULL.',
  );
  expect(cellError(column('integer'), false, DEFAULT_VALUE)).toBe(
    'amount has no DEFAULT and does not accept NULL.',
  );
  expect(
    cellError(column('integer', { default: '0' }), false, DEFAULT_VALUE),
  ).toBeNull();
  expect(cellError(column('integer'), false, textValue('x1'))).toBe(
    'amount: "x1" is not an integer.',
  );
  expect(cellError(column('integer'), false, textValue('10'))).toBeNull();
  expect(
    cellError(column('text', { nullable: true }), false, NULL_VALUE),
  ).toBeNull();
});

it('summarises the first problems and counts the rest', () => {
  const problems = Array.from({ length: 7 }, (_, index) => ({
    row: `Row ${String(index + 1)}`,
    message: 'amount: "x" is not an integer.',
  }));
  const text = summarize(problems);
  expect(text.startsWith('Nothing was staged. Row 1 · amount')).toBe(true);
  expect(text).toContain('Row 5');
  expect(text).not.toContain('Row 6');
  expect(text.endsWith('… and 2 more')).toBe(true);
});

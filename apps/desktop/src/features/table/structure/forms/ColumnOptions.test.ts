// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { optionChanges } from './ColumnOptions';

it('keeps current anchors and explicit conversion expressions in typed changes', () => {
  expect(
    optionChanges('value', 'integer', {
      expression: ' value::integer ',
      position: 'after:key',
    }),
  ).toEqual([
    {
      kind: 'columnTypeUsing',
      column: 'value',
      dataType: 'integer',
      expression: 'value::integer',
    },
    { kind: 'reorderColumn', column: 'value', after: 'key' },
  ]);
  expect(
    optionChanges('value', 'text', { expression: '', position: 'first' }),
  ).toEqual([{ kind: 'reorderColumn', column: 'value', after: null }]);
  expect(
    optionChanges('value', 'text', { expression: '', position: '' }),
  ).toEqual([]);
});

it('clears an existing conversion without discarding its chosen type', () => {
  expect(
    optionChanges('value', 'integer', { expression: '', position: '' }, true),
  ).toEqual([{ kind: 'columnType', column: 'value', dataType: 'integer' }]);
});

// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { whereClause, withParams } from './whereClause';

it('extracts the generated WHERE clause and lists its parameters', () => {
  expect(
    whereClause(
      'SELECT * FROM "t" WHERE ("id" > $1) AND (x IN (SELECT y FROM z LIMIT 3)) LIMIT 1 OFFSET 0',
    ),
  ).toBe('WHERE ("id" > $1) AND (x IN (SELECT y FROM z LIMIT 3))');
  expect(whereClause('SELECT * FROM "t" LIMIT 1 OFFSET 0')).toBeNull();
  expect(withParams('WHERE ("a" = $1)', ['x"y', null])).toBe(
    'WHERE ("a" = $1)\n-- $1 = "x\\"y"\n-- $2 = NULL',
  );
});

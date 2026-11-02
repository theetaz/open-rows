// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import {
  connectionLink,
  filterLink,
  linkableFilters,
  objectLink,
  tableLink,
} from './linkText';

const profile = {
  id: '0192d3c4-5b6a-7c8d-9e0f-a1b2c3d4e5f6',
  name: 'Local PG',
};

/** The exact text the native parser test (`links::tests`) also parses. */
const TABLE_LINK =
  'openrows://table?id=0192d3c4-5b6a-7c8d-9e0f-a1b2c3d4e5f6&name=Local+PG&database=app&schema=public&object=orders&kind=view' +
  '&filter=column%3Dstatus%26op%3Dequal%26value%3Dpaid%2B%2526%2Blate%26second%3D' +
  '&filter=column%3Dtotal%26op%3Dbetween%26value%3D1%26second%3D9';

it('names the connection by id and name only, never by host or credentials', () => {
  const link = connectionLink(profile, 'app');
  expect(link).toBe(
    'openrows://connection?id=0192d3c4-5b6a-7c8d-9e0f-a1b2c3d4e5f6&name=Local+PG&database=app',
  );
  expect(objectLink(profile, { database: 'app', schema: 'sales' })).toBe(
    'openrows://object?id=0192d3c4-5b6a-7c8d-9e0f-a1b2c3d4e5f6&name=Local+PG&database=app&schema=sales',
  );
});

it('encodes each filter as its own form-encoded value', () => {
  expect(
    tableLink(
      profile,
      { database: 'app', schema: 'public', name: 'orders', kind: 'view' },
      [
        {
          column: 'status',
          operator: 'equal',
          value: 'paid & late',
          second: '',
        },
        { column: 'total', operator: 'between', value: '1', second: '9' },
      ],
    ),
  ).toBe(TABLE_LINK);
});

it('links only filters that must all match and carry no SQL', () => {
  const equal = {
    column: 'id',
    operator: 'equal' as const,
    value: '1',
    second: '',
  };
  const view = { applied: [equal, equal], combinator: 'and' as const };
  expect(linkableFilters(view)).toEqual([equal, equal]);
  expect(linkableFilters({ ...view, combinator: 'or' })).toBeNull();
  expect(
    linkableFilters({
      ...view,
      applied: [{ ...equal, operator: 'raw', value: '1=1' }],
    }),
  ).toBeNull();
  expect(
    linkableFilters({
      ...view,
      combinator: 'or',
      appliedGroups: [
        { combinator: 'and', filters: [equal] },
        { combinator: 'and', filters: [equal] },
      ],
    }),
  ).toBeNull();
  expect(
    filterLink(
      profile,
      { database: 'app', schema: 'public', name: 'orders' },
      { applied: [], combinator: 'and' },
    ),
  ).toContain('openrows://table?');
});

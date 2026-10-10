// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { scopeOptions } from './scopeModel';

const a = { schema: 'a', name: 'one' };
const b = { schema: 'b', name: 'two' };
const catalog = [
  { name: 'a', tables: [a] },
  { name: 'b', tables: [b] },
];

it('keeps pure schemas and tables literal and expands mixed scopes without duplicates', () => {
  expect(
    scopeOptions({ all: true, schemas: ['a'], tables: [b] }, catalog),
  ).toEqual({ schemas: [], tables: [] });
  expect(
    scopeOptions({ all: false, schemas: ['a'], tables: [a] }, catalog),
  ).toEqual({ schemas: ['a'], tables: [] });
  expect(
    scopeOptions({ all: false, schemas: [], tables: [b] }, catalog),
  ).toEqual({ schemas: [], tables: [b] });
  expect(
    scopeOptions({ all: false, schemas: ['a'], tables: [a, b] }, catalog),
  ).toEqual({ schemas: [], tables: [a, b] });
});

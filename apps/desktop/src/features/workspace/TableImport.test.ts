// SPDX-License-Identifier: Apache-2.0
import { expect, it, vi } from 'vitest';
import { importOpener, importTarget } from './TableImport';

const selected = { database: 'shop', schema: 'public' };

it('imports into the tree object, the table tab, or a new table of the schema', () => {
  const object = {
    database: 'shop',
    schema: 'sales',
    name: 'orders',
    kind: 'table' as const,
    arguments: null,
  };
  const tab = { database: 'shop', schema: 'public', name: 'users' };
  expect(importTarget(object, tab, selected)).toEqual({
    database: 'shop',
    schema: 'sales',
    table: 'orders',
  });
  expect(importTarget(undefined, tab, selected)).toEqual({
    database: 'shop',
    schema: 'public',
    table: 'users',
  });
  expect(importTarget(undefined, undefined, selected)).toEqual({
    database: 'shop',
    schema: 'public',
    table: null,
  });
  expect(
    importTarget(undefined, undefined, { database: '', schema: 'public' }),
  ).toBeNull();
  const open = vi.fn();
  importOpener({ database: '', schema: '' }, undefined, open)();
  expect(open).not.toHaveBeenCalled();
  importOpener(selected, tab, open)();
  expect(open).toHaveBeenCalledWith({
    database: 'shop',
    schema: 'public',
    table: 'users',
  });
});

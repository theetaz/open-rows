// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { CATALOG_OBJECTS, syntheticCatalog } from './completionCatalog';

it('serves a warm 50 000-object catalog with the benchmark triggers resolvable', async () => {
  const catalog = syntheticCatalog();
  const objects = await catalog.objects();
  expect(objects).toHaveLength(CATALOG_OBJECTS);
  expect(await catalog.objects()).toBe(objects);
  expect((await catalog.schemas()).map((schema) => schema.name)).toContain(
    'sales',
  );
  expect(await catalog.objectsIn('sales')).toHaveLength(CATALOG_OBJECTS / 5);
  expect(await catalog.objectsIn('missing')).toEqual([]);
  expect(objects).toContainEqual({
    schema: 'public',
    name: 'orders_00010',
    kind: 'table',
  });
  const columns = await catalog.columns('public', 'orders_00010');
  expect(columns[0]).toEqual({ name: 'id', dataType: 'bigint' });
  expect(columns).toHaveLength(40);
});

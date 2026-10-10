// SPDX-License-Identifier: Apache-2.0
import { beforeEach, expect, it, vi } from 'vitest';
import { schemaDiagram, tableDetail } from '@openrows/ipc';
import { sampleDetail } from '../../../test/structureStub';
import { invalidateLinks, readLinks } from './linkMetadata';
import { detailLinks, linkLabel, noLinks } from './foreignKeys';

vi.mock('@openrows/ipc', async () =>
  (await import('../../../test/ipcStub')).ipcStub(),
);
const target = {
  profileId: 'links-test',
  database: 'shop',
  schema: 'a',
  table: 'orders',
};
const detail = {
  ...sampleDetail,
  constraints: [
    {
      name: 'customer',
      kind: 'foreign' as const,
      definition: '',
      columns: ['customer_id'],
      references: { schema: 'b', table: 'customers', columns: ['id'] },
    },
  ],
};

beforeEach(() => {
  invalidateLinks(target.profileId);
  vi.mocked(tableDetail).mockClear().mockResolvedValue(detail);
  vi.mocked(schemaDiagram).mockClear();
});

it('uses qualified destinations and separates schemas and sessions in the 60-second cache', async () => {
  let now = 0;
  vi.spyOn(Date, 'now').mockImplementation(() => now);
  expect(
    (await readLinks(target, 'one', 'postgres')).outgoing.map(linkLabel),
  ).toEqual(['b.customers (id)']);
  await readLinks(target, 'one', 'postgres');
  expect(tableDetail).toHaveBeenCalledTimes(1);
  await readLinks({ ...target, schema: 'other' }, 'one', 'postgres');
  await readLinks(target, 'two', 'postgres');
  expect(tableDetail).toHaveBeenCalledTimes(3);
  now = 60_001;
  await readLinks(target, 'one', 'postgres');
  expect(tableDetail).toHaveBeenCalledTimes(4);
  invalidateLinks(target.profileId);
  await readLinks(target, 'one', 'postgres');
  expect(tableDetail).toHaveBeenCalledTimes(5);
  vi.mocked(Date.now).mockRestore();
});

it('retries failed reads and excludes MySQL cross-database keys', async () => {
  vi.mocked(schemaDiagram).mockRejectedValueOnce(new Error('offline'));
  await expect(readLinks(target, 'one', 'postgres')).rejects.toThrow('offline');
  expect((await readLinks(target, 'one', 'postgres')).outgoing).toHaveLength(1);
  expect((await readLinks(target, 'one', 'mysql')).outgoing).toHaveLength(0);
  expect(
    detailLinks(
      {
        ...detail,
        constraints: [
          {
            name: 'customer',
            kind: 'foreign',
            definition: '',
            columns: ['id', 'region'],
            references: {
              schema: 'b',
              table: 'customers',
              columns: ['id', 'region'],
            },
          },
        ],
      },
      noLinks,
      'a',
    ).outgoing[0]?.targetColumns,
  ).toEqual(['id', 'region']);
});

it('shares bounded ER metadata across tables of one schema', async () => {
  await readLinks(target, 'one', 'postgres');
  await readLinks({ ...target, table: 'other' }, 'one', 'postgres');
  expect(tableDetail).toHaveBeenCalledTimes(2);
  expect(schemaDiagram).toHaveBeenCalledTimes(1);
});

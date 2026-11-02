// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import type { OpenLinkDto } from '@openrows/ipc';
import { profile } from '../../test/ipcStub';
import { linkRows, resolveLink } from './openLink';

const local = profile('orders-local');
const prod = profile('analytics', {
  id: 'id-prod',
  env: { kind: 'prod', label: 'Production', color: '#dc2626' },
  user: 'report-reader',
});

const table = (connection: OpenLinkDto['connection']): OpenLinkDto => ({
  kind: 'table',
  connection,
  database: 'app',
  schema: 'public',
  object: { name: 'orders', kind: 'table' },
  filters: [{ column: 'status', operator: 'equal', value: 'paid', second: '' }],
});

it('resolves the id first, then a unique name', () => {
  const byId = resolveLink(table({ id: 'id-prod', name: 'renamed' }), [
    local,
    prod,
  ]);
  expect('profile' in byId && byId.profile.id).toBe('id-prod');
  const byName = resolveLink(table({ id: null, name: 'orders-local' }), [
    local,
    prod,
  ]);
  expect('profile' in byName && byName.profile.name).toBe('orders-local');
  expect(
    resolveLink(table({ id: null, name: 'orders-local' }), [local, local]),
  ).toMatchObject({ error: expect.stringContaining('Several') as string });
  expect(resolveLink(table({ id: null, name: 'nope' }), [local])).toMatchObject(
    { error: expect.stringContaining('No saved connection') as string },
  );
});

it('previews where it connects and what it opens, without credentials', () => {
  const resolved = resolveLink(table({ id: 'id-prod', name: null }), [prod]);
  if (!('profile' in resolved)) throw new Error(resolved.error);
  const rows = linkRows(resolved);
  expect(rows).toEqual([
    ['Connection', 'analytics (Production)'],
    ['Server', 'postgres · localhost:5432'],
    ['Database', 'app'],
    ['Schema', 'public'],
    ['Opens', 'orders (table)'],
    ['Filter', 'status = paid'],
  ]);
  expect(JSON.stringify(rows)).not.toContain('report-reader');
});

it('previews a MongoDB collection link as a collection, without a schema row', () => {
  const mongo = profile('catalog', {
    id: 'id-mongo',
    engine: 'mongodb',
    port: 27017,
  });
  const link: OpenLinkDto = {
    kind: 'table',
    connection: { id: 'id-mongo', name: 'catalog' },
    database: 'shop',
    schema: 'shop',
    object: { name: 'orders', kind: 'table' },
    filters: [
      { column: 'status', operator: 'equal', value: 'paid', second: '' },
    ],
  };
  const resolved = resolveLink(link, [mongo]);
  if (!('profile' in resolved)) throw new Error(resolved.error);
  expect(linkRows(resolved)).toEqual([
    ['Connection', 'catalog (local)'],
    ['Server', 'mongodb · localhost:27017'],
    ['Database', 'shop'],
    ['Opens', 'orders (collection)'],
    ['Filter', 'status = paid'],
  ]);
});

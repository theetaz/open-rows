// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import type { SchemaDiagramDto } from '@openrows/ipc';
import {
  jumpFilters,
  linkedColumns,
  linkLabel,
  linkOf,
  tableLinks,
} from './foreignKeys';
import { seededView, seedFilters, takeSeed } from './seeds';
import { initialView } from '../model';

const column = (name: string, primaryKey = false) => ({
  name,
  dataType: 'int4',
  nullable: !primaryKey,
  primaryKey,
});

const diagram: SchemaDiagramDto = {
  tables: [
    { name: 'orgs', columns: [column('id', true), column('region', true)] },
    {
      name: 'users',
      columns: [column('id', true), column('org_id'), column('region')],
    },
    { name: 'notes', columns: [column('id', true), column('user_id')] },
  ],
  foreignKeys: [
    {
      name: 'users_org',
      table: 'users',
      columns: ['org_id', 'region'],
      targetTable: 'orgs',
      targetColumns: [],
    },
    {
      name: 'notes_user',
      table: 'notes',
      columns: ['user_id'],
      targetTable: 'users',
      targetColumns: ['id'],
    },
  ],
  truncated: false,
  limit: 500,
};

it('reads outgoing and incoming keys, resolving implicit targets to the primary key', () => {
  const links = tableLinks(diagram, 'app', 'users');
  expect(links.outgoing).toEqual([
    {
      name: 'users_org',
      schema: 'app',
      table: 'orgs',
      columns: ['org_id', 'region'],
      targetColumns: ['id', 'region'],
    },
  ]);
  expect(links.incoming).toEqual([
    {
      name: 'notes_user',
      schema: 'app',
      table: 'notes',
      columns: ['id'],
      targetColumns: ['user_id'],
    },
  ]);
  expect([...linkedColumns(links)]).toEqual(['org_id', 'region']);
  expect(linkOf(links, 'region')?.table).toBe('orgs');
  expect(linkOf(links, 'id')).toBeUndefined();
  expect(links.outgoing.map(linkLabel)).toEqual(['orgs (id, region)']);
});

it('jumps with one bound equality filter per key column, never on NULL keys', () => {
  const [link] = tableLinks(diagram, 'app', 'users').outgoing;
  if (!link) throw new Error('missing key');
  const values = new Map<string, string | null>([
    ['org_id', '42'],
    ['region', "eu'; --"],
  ]);
  expect(jumpFilters(link, values)).toEqual([
    { column: 'id', operator: 'equal', value: '42', second: '' },
    { column: 'region', operator: 'equal', value: "eu'; --", second: '' },
  ]);
  expect(
    jumpFilters(
      link,
      new Map([
        ['org_id', '42'],
        ['region', null],
      ]),
    ),
  ).toBeNull();
  expect(jumpFilters(link, new Map([['org_id', '42']]))).toBeNull();
});

it('hands a jump to the target tab once, which shows the filters applied', () => {
  const filters = [
    { column: 'id', operator: 'equal' as const, value: '7', second: '' },
  ];
  seedFilters('k', filters);
  expect(takeSeed('k')).toBe(filters);
  expect(takeSeed('k')).toBeUndefined();
  expect(
    seededView({ ...initialView, nextRow: 3, offset: 600 }, filters),
  ).toMatchObject({
    rows: [{ ...filters[0], id: 3, enabled: true }],
    applied: filters,
    combinator: 'and',
    filtersOpen: true,
    offset: 0,
    nextRow: 4,
  });
});

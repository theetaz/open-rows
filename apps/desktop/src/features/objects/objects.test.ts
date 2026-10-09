// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { postgresObjects } from '../../test/ipcStub';
import { objectCommandRequest, objectCommandSupported } from './commands';
import { dropChange } from './DropSheet';
import { containerEntries, objectEntries } from './menus';
import {
  createTableChange,
  emptyTable,
  renameColumn,
  tableProblem,
} from './newTable';

const place = { database: 'orders', schema: 'public' };
const table = { ...place, name: 'orders' };

it('maps palette commands to editors of the browsed schema or the active table', () => {
  expect(objectCommandRequest('new-table', { place, table: null })).toEqual({
    kind: 'newTable',
    ...place,
  });
  expect(objectCommandRequest('drop-table', { place, table })).toEqual({
    kind: 'drop',
    target: { object: 'table', ...table },
  });
  expect(
    objectCommandRequest('rename-table', { place, table: null }),
  ).toBeNull();
  expect(
    objectCommandRequest('new-materialized-view', { place, table }),
  ).toMatchObject({
    kind: 'view',
    name: null,
    materialized: true,
  });
  expect(objectCommandRequest('drop-database', { place, table })).toEqual({
    kind: 'drop',
    target: { object: 'database', database: 'orders', name: 'orders' },
  });
  expect(
    objectCommandRequest('rename-schema', {
      place: { ...place, schema: '' },
      table: null,
    }),
  ).toBeNull();
  const mysql = { ...postgresObjects, schemas: false, renameDatabases: false };
  expect(objectCommandSupported('new-schema', mysql)).toBe(false);
  expect(objectCommandSupported('drop-database', mysql)).toBe(true);
});

it('disables what the engine cannot do and marks drops as dangerous', () => {
  const sqlite = {
    ...postgresObjects,
    routines: false,
    databases: false,
    renameDatabases: false,
    schemas: false,
    materializedViews: false,
  };
  const routine = objectEntries(
    { ...place, name: 'calc', kind: 'function', arguments: 'integer' },
    sqlite,
  );
  expect(routine.every((entry) => entry.request === null)).toBe(true);
  const tableItems = objectEntries(
    { ...table, kind: 'table', arguments: null },
    sqlite,
  );
  expect(tableItems.map((entry) => entry.label)).toEqual([
    'New Table…',
    'Rename Table…',
    'Duplicate Table…',
    'New Trigger…',
    'Truncate Table…',
    'Drop Table…',
  ]);
  expect(tableItems.filter((entry) => entry.danger)).toHaveLength(2);
  const containers = containerEntries(place, sqlite);
  expect(containers.every((entry) => entry.request === null)).toBe(true);
  expect(containers[0]?.reason).toBe('Not supported by this database');
});

it('builds drop requests per object kind', () => {
  expect(dropChange({ object: 'materializedView', ...table }, true)).toEqual({
    kind: 'dropView',
    target: { schema: 'public', name: 'orders' },
    materialized: true,
    cascade: true,
  });
  expect(
    dropChange(
      { object: 'trigger', ...place, table: 'orders', name: 'touch' },
      false,
    ),
  ).toEqual({
    kind: 'dropTrigger',
    target: { schema: 'public', name: 'orders' },
    name: 'touch',
  });
  expect(
    dropChange(
      { object: 'function', ...place, name: 'calc', arguments: 'integer' },
      false,
    ),
  ).toMatchObject({
    kind: 'dropRoutine',
    routine: 'function',
    arguments: 'integer',
  });
});

it('turns a new-table draft into one create request', () => {
  const draft = emptyTable('postgres', postgresObjects);
  expect(tableProblem(draft)).toBe('Name the table.');
  const named = { ...draft, name: 'notes' };
  expect(tableProblem(named)).toBeNull();
  const renamed = renameColumn(
    {
      ...named,
      indexes: [
        {
          name: null,
          columns: ['id'],
          unique: true,
          method: 'btree',
          descending: ['id'],
          predicate: null,
        },
      ],
    },
    'id',
    'note_id',
  );
  expect(renamed.primaryKey).toEqual(['note_id']);
  expect(renamed.indexes[0]?.descending).toEqual(['note_id']);
  expect(createTableChange(named, 'public')).toEqual({
    kind: 'createTable',
    target: { schema: 'public', name: 'notes' },
    definition: {
      columns: [
        {
          name: 'id',
          dataType: 'bigint',
          nullable: false,
          default: { kind: 'none' },
          comment: null,
          identity: 'always',
          collation: null,
          charset: null,
        },
      ],
      primaryKey: ['id'],
      indexes: [],
      foreignKeys: [],
      checks: [],
      comment: null,
    },
  });
  const twice = { ...named, columns: [...named.columns, ...named.columns] };
  expect(tableProblem(twice)).toBe('Two columns are named id.');
});

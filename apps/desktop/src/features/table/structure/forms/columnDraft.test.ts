// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import type { ColumnRow } from '../effective';
import {
  addColumnChange,
  columnChanges,
  columnProblem,
  columnType,
  draftFromRow,
  emptyColumn,
  splitType,
} from './columnDraft';
import { identityOptions, indexMethods, typeSpecs } from './dialect';
import { postgresObjects } from '../../../../test/ipcStub';

const row: ColumnRow = {
  id: 'price',
  column: 'price',
  name: 'price',
  dataType: 'numeric(10, 2)',
  nullable: true,
  default: "'0'",
  generated: 'no',
  comment: null,
  check: null,
  foreignKey: null,
  dropped: false,
  changed: [],
};

it('splits and composes types with length, precision and scale', () => {
  expect(splitType('character varying(40)')).toEqual({
    type: 'character varying',
    length: '40',
    scale: '',
  });
  expect(splitType('numeric(10, 2)')).toEqual({
    type: 'numeric',
    length: '10',
    scale: '2',
  });
  expect(splitType('int(11) unsigned')).toEqual({
    type: 'int unsigned',
    length: '11',
    scale: '',
  });
  expect(splitType('text[]')).toEqual({
    type: 'text[]',
    length: '',
    scale: '',
  });
  const draft = {
    ...emptyColumn('a', 'postgres'),
    type: 'numeric',
    length: '12',
    scale: '3',
  };
  expect(columnType(draft)).toBe('numeric(12,3)');
  expect(columnType({ ...draft, type: 'int unsigned', scale: '' })).toBe(
    'int(12) unsigned',
  );
});

it('stages only the properties that changed, the rename last', () => {
  const original = draftFromRow(row);
  expect(original).toMatchObject({
    defaultMode: 'literal',
    defaultText: '0',
    length: '10',
  });
  const edited = {
    ...original,
    name: 'amount',
    length: '12',
    nullable: false,
    collation: 'C',
    comment: 'Gross',
  };
  expect(columnChanges(edited, 'price', original)).toEqual([
    { kind: 'columnType', column: 'price', dataType: 'numeric(12,2)' },
    { kind: 'columnCollation', column: 'price', charset: null, collation: 'C' },
    { kind: 'columnNullable', column: 'price', nullable: false },
    { kind: 'columnComment', column: 'price', comment: 'Gross' },
    { kind: 'renameColumn', column: 'price', name: 'amount' },
  ]);
  expect(columnChanges(original, 'price', original)).toEqual([]);
});

it('adds new columns with numbering and refuses incomplete forms', () => {
  const draft = {
    ...emptyColumn('id', 'mysql'),
    type: 'bigint unsigned',
    nullable: false,
    identity: 'autoIncrement' as const,
  };
  expect(addColumnChange(draft)).toEqual({
    kind: 'addColumn',
    name: 'id',
    dataType: 'bigint unsigned',
    nullable: false,
    default: { kind: 'none' },
    comment: null,
    identity: 'autoIncrement',
    collation: null,
    charset: null,
  });
  expect(columnProblem({ ...draft, name: ' ' })).toBe('Name the column.');
  expect(
    columnProblem({ ...draft, defaultMode: 'expression', defaultText: '1' }),
  ).toBe('A numbered column has no default.');
  expect(columnProblem({ ...draft, scale: '2' })).toBe(
    'A scale needs a precision.',
  );
});

it('offers each engine its own types, index methods and numbering', () => {
  expect(typeSpecs('postgres').map((spec) => spec.name)).toContain('jsonb');
  expect(typeSpecs('mysql').map((spec) => spec.name)).toContain('datetime');
  expect(typeSpecs('sqlite')[0]?.name).toBe('INTEGER');
  expect(indexMethods('mysql').map((method) => method.value)).toEqual([
    'btree',
    'hash',
  ]);
  expect(indexMethods('sqlite')).toHaveLength(1);
  expect(
    identityOptions('postgres', postgresObjects).map((o) => o.value),
  ).toEqual(['none', 'always', 'byDefault']);
  expect(
    identityOptions('mariadb', postgresObjects).map((o) => o.value),
  ).toEqual(['none', 'autoIncrement']);
  expect(
    identityOptions('sqlite', { ...postgresObjects, identity: false }),
  ).toHaveLength(1);
});

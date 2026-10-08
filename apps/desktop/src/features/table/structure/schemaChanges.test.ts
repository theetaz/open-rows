// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import type { SchemaChangeDto } from '@openrows/ipc';
import { emptyHistory, travel } from '../editHistory';
import { sampleDetail } from '../../../test/structureStub';
import { columnRows, indexRows, typedDefault } from './effective';
import { formatBytes, formatRows } from './format';
import {
  defaultText,
  describe,
  replaceStaged,
  stageSchema,
  unstage,
} from './schemaChanges';
import type { SchemaHistory } from './schemaChanges';

const base = { detail: sampleDetail, table: 'orders' };
const stageAll = (changes: SchemaChangeDto[]): SchemaHistory =>
  changes.reduce<SchemaHistory>(
    (history, change) => stageSchema(history, change, base),
    emptyHistory,
  );
const kinds = (history: SchemaHistory): string[] =>
  history.current.map((item) => item.change.kind);

it('keeps one change per property and drops changes back to the catalog value', () => {
  const history = stageAll([
    { kind: 'columnType', column: 'name', dataType: 'varchar(40)' },
    { kind: 'columnType', column: 'name', dataType: 'citext' },
    { kind: 'columnNullable', column: 'name', nullable: true },
    { kind: 'columnComment', column: 'name', comment: 'Display name' },
    { kind: 'renameTable', name: 'orders' },
    {
      kind: 'columnDefault',
      column: 'name',
      default: { kind: 'expression', sql: " 'anon'::text " },
    },
  ]);
  expect(history.current).toEqual([
    {
      id: 'columnType:name',
      change: { kind: 'columnType', column: 'name', dataType: 'citext' },
    },
  ]);
  const back = stageSchema(
    history,
    { kind: 'columnType', column: 'name', dataType: 'text' },
    base,
  );
  expect(back.current).toEqual([]);
  expect(travel(back, 'undo').current).toEqual(history.current);
});

it('dropping a column replaces its staged edits; empty comments mean none', () => {
  const history = stageAll([
    { kind: 'renameColumn', column: 'name', name: 'title' },
    { kind: 'columnComment', column: 'customer_id', comment: '' },
    { kind: 'dropColumn', column: 'name' },
  ]);
  expect(kinds(history)).toEqual(['dropColumn']);
  const restored = unstage(history, 'dropColumn:name');
  expect(restored.current).toEqual([]);
});

it('edits staged new columns in place and overlays them as rows', () => {
  let history = stageAll([
    {
      kind: 'addColumn',
      name: 'notes',
      dataType: 'text',
      nullable: true,
      default: { kind: 'none' },
      comment: null,
    },
    { kind: 'renameColumn', column: 'name', name: 'title' },
    { kind: 'dropColumn', column: 'customer_id' },
  ]);
  const id = history.current[0]?.id ?? '';
  history = replaceStaged(history, id, {
    kind: 'addColumn',
    name: 'memo',
    dataType: 'text',
    nullable: false,
    default: { kind: 'literal', value: "it's" },
    comment: null,
  });
  const rows = columnRows(sampleDetail, history.current);
  expect(rows.map((row) => row.name)).toEqual([
    'id',
    'title',
    'customer_id',
    'memo',
  ]);
  expect(rows[1]?.changed).toEqual(['name']);
  expect(rows[2]?.dropped).toBe(true);
  expect(rows[3]).toMatchObject({
    column: null,
    nullable: false,
    default: "'it''s'",
  });
});

it('marks dropped and new indexes', () => {
  const history = stageAll([
    { kind: 'dropIndex', name: 'orders_name_idx' },
    {
      kind: 'createIndex',
      name: null,
      columns: ['name', 'id'],
      unique: true,
      method: 'btree',
    },
  ]);
  const rows = indexRows(sampleDetail, history.current);
  expect(rows.map((row) => [row.name, row.dropped, row.added])).toEqual([
    ['orders_pkey', false, false],
    ['orders_name_idx', true, false],
    ['', false, true],
  ]);
  expect(rows[2]?.columns).toBe('name, id');
});

it('types defaults explicitly and formats sizes like TablePlus', () => {
  expect(typedDefault('  ', false)).toEqual({ kind: 'none' });
  expect(typedDefault(' now() ', false)).toEqual({
    kind: 'expression',
    sql: 'now()',
  });
  expect(typedDefault('', true)).toEqual({ kind: 'literal', value: '' });
  expect(defaultText({ kind: 'literal', value: "a'b" })).toBe("'a''b'");
  expect(describe({ kind: 'dropIndex', name: 'i' })).toBe('Delete index i');
  expect(formatBytes(82944)).toBe('81 KB');
  expect(formatBytes(1_241_088)).toBe('1 MB');
  expect(formatBytes(512)).toBe('512 bytes');
  expect(formatBytes(null)).toBe('—');
  expect(formatRows(1788)).toBe('1,788');
});

it('ignores staging while a commit is pending', () => {
  const pending = { ...emptyHistory, pending: true };
  expect(
    stageSchema(pending, { kind: 'dropColumn', column: 'name' }, base),
  ).toBe(pending);
});

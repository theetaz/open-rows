// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import type { TableColumnDto } from '@openrows/ipc';
import { samplePreview } from '../../test/importStub';
import { confirmation } from './ImportSheet';
import {
  defaultColumns,
  defaultMapping,
  draftProblem,
  importRequest,
  initialDraft,
  previewTypes,
} from './model';
import type { ImportContext, ImportDraft } from './model';

const context: ImportContext = {
  profileId: 'profile-1',
  sessionId: 'session-1',
  database: 'shop',
  schema: 'public',
  table: 'users',
  readOnly: false,
};

const column = (name: string, dataType: string): TableColumnDto => ({
  name,
  dataType,
  nullable: true,
  default: null,
  comment: null,
  primaryKey: false,
  enumLabels: null,
});

const targets = [
  column('id', 'integer'),
  column('full_name', 'text'),
  column('origin', 'text'),
];

function ready(patch: Partial<ImportDraft> = {}): ImportDraft {
  return {
    ...initialDraft(context),
    file: { fileId: 'file-1', fileName: 'people.csv' },
    options: samplePreview.options,
    ...patch,
  };
}

it('maps target columns to loosely matching source names', () => {
  const sources = ['ID', 'Full Name'].map((name) => ({
    name,
    category: 'text' as const,
    nullable: true,
    suggestedType: 'text',
  }));
  expect(defaultMapping(targets, sources)).toEqual({
    id: { kind: 'column', index: 0 },
    full_name: { kind: 'column', index: 1 },
    origin: { kind: 'skip' },
  });
  expect(defaultColumns(sources)[1]).toEqual({
    include: true,
    name: 'Full Name',
    type: 'text',
    key: false,
  });
});

it('previews each source column against the type it feeds', () => {
  const draft = ready({
    mapping: {
      id: { kind: 'column', index: 1 },
      full_name: { kind: 'skip' },
    },
  });
  expect(previewTypes(draft, targets, 2)).toEqual([null, 'integer']);
  const created = ready({
    target: 'new',
    columns: [
      { include: true, name: 'id', type: 'bigint', key: true },
      { include: false, name: 'name', type: 'text', key: false },
    ],
  });
  expect(previewTypes(created, [], 2)).toEqual(['bigint', null]);
});

it('explains what is missing before a request can be built', () => {
  expect(draftProblem(initialDraft(context))).toBe('Choose a file to import.');
  expect(draftProblem(ready())).toBe('Map at least one column.');
  const mapped = ready({ mapping: { id: { kind: 'column', index: 0 } } });
  expect(draftProblem(mapped)).toBeNull();
  expect(draftProblem({ ...mapped, conflict: 'update' })).toMatch(/key/);
  expect(draftProblem({ ...mapped, batchSize: '0' })).toMatch(/batch/);
  expect(draftProblem({ ...mapped, table: ' ' })).toBe('Enter the table name.');
  const created = ready({
    target: 'new',
    table: 'people',
    columns: [
      { include: true, name: 'a', type: 'text', key: false },
      { include: true, name: 'a', type: 'text', key: false },
    ],
  });
  expect(draftProblem(created)).toBe('Column names must differ.');
});

it('builds requests for existing and new tables', () => {
  const existing = ready({
    mapping: {
      id: { kind: 'column', index: 0 },
      full_name: { kind: 'null' },
      origin: { kind: 'constant', value: 'csv' },
    },
    key: ['id'],
    conflict: 'skip',
    truncate: true,
  });
  const request = importRequest(existing, context, 'check');
  expect(request).toMatchObject({
    target: { profileId: 'profile-1', database: 'shop', table: 'users' },
    columns: [
      { target: 'id', source: { kind: 'column', index: 0 } },
      { target: 'full_name', source: { kind: 'constant', value: null } },
      { target: 'origin', source: { kind: 'constant', value: 'csv' } },
    ],
    create: null,
    key: ['id'],
    conflict: 'skip',
    truncate: true,
    mode: 'check',
    batchSize: 5000,
  });
  const created = ready({
    target: 'new',
    table: ' people ',
    truncate: true,
    conflict: 'update',
    columns: [
      { include: true, name: ' id ', type: 'bigint', key: true },
      { include: false, name: 'skip', type: 'text', key: false },
      { include: true, name: 'name', type: 'text', key: false },
    ],
  });
  expect(importRequest(created, context, 'import')).toMatchObject({
    target: { table: 'people' },
    columns: [
      { target: 'id', source: { kind: 'column', index: 0 } },
      { target: 'name', source: { kind: 'column', index: 2 } },
    ],
    create: { types: ['bigint', 'text'], primaryKey: ['id'] },
    conflict: 'error',
    truncate: false,
  });
  expect(importRequest(initialDraft(context), context, 'import')).toBeNull();
});

it('confirms what the import will write, create and delete', () => {
  const truncating = ready({ truncate: true });
  expect(confirmation(truncating, context)).toEqual({
    title: 'Import into users?',
    message:
      'Rows from people.csv will be written to public.users. Every existing row of the table is deleted first.',
    action: 'Import',
  });
  const created = ready({ target: 'new', table: 'people' });
  expect(confirmation(created, context).message).toContain(
    'The table is created first.',
  );
});

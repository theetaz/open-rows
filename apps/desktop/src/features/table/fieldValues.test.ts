// SPDX-License-Identifier: Apache-2.0
import { describe, expect, it } from 'vitest';
import type { TableColumnDto } from '@openrows/ipc';
import {
  DEFAULT_VALUE,
  NULL_VALUE,
  stagedDisplay,
  textValue,
  unchanged,
} from './cellValue';
import { stagedCell } from './EditedSource';
import { fieldActions, fieldValueError, fileEncoding } from './fieldActions';
import { formatJson } from './jsonFormat';
import type { StagedEdit } from './editHistory';
import type { RowField } from './row';
import { stagedEditFor, withStaged } from './selectedRow';

const column = (facts: Partial<TableColumnDto>): TableColumnDto => ({
  name: 'c',
  dataType: 'text',
  nullable: true,
  default: null,
  comment: null,
  primaryKey: false,
  enumLabels: null,
  ...facts,
});

describe('staged values', () => {
  it('compares NULL, EMPTY, text and DEFAULT with the exact original', () => {
    expect(unchanged(NULL_VALUE, null)).toBe(true);
    expect(unchanged(NULL_VALUE, '')).toBe(false);
    expect(unchanged(textValue(''), null)).toBe(false);
    expect(unchanged(textValue(''), '')).toBe(true);
    expect(unchanged(textValue('NULL'), null)).toBe(false);
    expect(unchanged(DEFAULT_VALUE, 'x')).toBe(false);
    expect(unchanged(DEFAULT_VALUE, null)).toBe(false);
    expect(unchanged(textValue('x'), undefined)).toBe(false);
  });

  it('paints NULL, DEFAULT and EMPTY as muted markers, text as itself', () => {
    expect(stagedDisplay(DEFAULT_VALUE)).toEqual({
      text: 'DEFAULT',
      placeholder: true,
    });
    expect(stagedCell(DEFAULT_VALUE, 'changed')).toEqual({
      text: 'DEFAULT',
      state: 'changed',
      marker: 'default',
    });
    expect(stagedCell(NULL_VALUE, 'changed').marker).toBe('null');
    expect(stagedCell(textValue(''), 'changed').text).toBe('EMPTY');
    expect(stagedCell(textValue('DEFAULT'), 'changed')).toEqual({
      text: 'DEFAULT',
      state: 'changed',
    });
  });
});

describe('JSON formatting', () => {
  it('keeps numbers, escapes and string contents exactly', () => {
    const source =
      ' { "n": 1.10e+400, "s": "a, \\"b\\": {[c]}", "e": [ ], "o": { } } ';
    expect(formatJson(source, false)).toBe(
      '{"n":1.10e+400,"s":"a, \\"b\\": {[c]}","e":[],"o":{}}',
    );
    expect(formatJson(source, true)).toBe(
      '{\n  "n": 1.10e+400,\n  "s": "a, \\"b\\": {[c]}",\n  "e": [],\n  "o": {}\n}',
    );
    expect(formatJson('[1,[2,{"a":null}]]', true)).toBe(
      '[\n  1,\n  [\n    2,\n    {\n      "a": null\n    }\n  ]\n]',
    );
    expect(formatJson('"text"', true)).toBe('"text"');
  });

  it('refuses invalid JSON', () => {
    expect(formatJson('{oops', true)).toBeNull();
    expect(formatJson('', false)).toBeNull();
    expect(formatJson("{'a':1}", false)).toBeNull();
  });
});

describe('field actions', () => {
  it('derives menu states from catalog facts', () => {
    expect(
      fieldActions(column({ nullable: false }), false, textValue('a')),
    ).toMatchObject({ null: false, default: false, empty: true, file: 'text' });
    expect(
      fieldActions(
        column({ dataType: 'integer', nullable: false, default: 'nextval(…)' }),
        false,
        textValue('1'),
      ),
    ).toMatchObject({ null: false, default: true, empty: false, file: null });
    expect(fieldActions(column({}), true, null)).toMatchObject({
      null: false,
      default: false,
      empty: false,
      file: null,
    });
    const json = fieldActions(
      column({ dataType: 'json' }),
      false,
      textValue('{"a": 1}'),
    );
    expect(json).toMatchObject({ pretty: '{\n  "a": 1\n}', minify: '{"a":1}' });
    expect(json.empty).toBe(false);
    expect(
      fieldActions(column({ dataType: 'jsonb' }), false, null).pretty,
    ).toBe(null);
  });

  it('chooses how files are read by column type', () => {
    expect(fileEncoding('bytea')).toBe('hex');
    expect(fileEncoding('character varying(255)')).toBe('text');
    expect(fileEncoding('public.citext')).toBe('text');
    expect(fileEncoding('jsonb')).toBe('text');
    expect(fileEncoding('xml')).toBe('text');
    expect(fileEncoding('text[]')).toBeNull();
    expect(fileEncoding('uuid')).toBeNull();
  });

  it('rechecks values when staging', () => {
    const notNull = column({ nullable: false });
    expect(fieldValueError(notNull, false, NULL_VALUE)).toMatch(/NULL/);
    expect(fieldValueError(notNull, false, DEFAULT_VALUE)).toMatch(/DEFAULT/);
    expect(fieldValueError(notNull, false, textValue(''))).toBeNull();
    expect(fieldValueError(column({}), false, DEFAULT_VALUE)).toBeNull();
    expect(fieldValueError(column({}), true, textValue('x'))).not.toBeNull();
    expect(fieldValueError(undefined, false, textValue('x'))).not.toBeNull();
  });
});

describe('inspector overlay', () => {
  const fields: RowField[] = [
    { index: 0, name: 'id', type: 'int4', text: '7', state: 'normal' },
    { index: 1, name: 'name', type: 'text', text: 'EMPTY', state: 'empty' },
    { index: 2, name: 'note', type: 'text', text: 'shown', state: 'normal' },
  ];
  const edit: StagedEdit = {
    id: '{"id":"7"}',
    number: 7,
    kind: 'update',
    original: { id: '7', name: '', note: 'exact' },
    values: { name: DEFAULT_VALUE },
  };

  it('finds the staged change by key and shows staged and exact values', () => {
    expect(stagedEditFor(fields, [edit], ['id'])).toBe(edit);
    expect(stagedEditFor(fields, [edit], [])).toBeUndefined();
    expect(
      stagedEditFor(fields, [{ ...edit, original: { id: '8' } }], ['id']),
    ).toBeUndefined();
    const shown = withStaged(fields, edit, null);
    expect(shown[1]).toMatchObject({
      text: 'DEFAULT',
      state: 'changed',
      value: DEFAULT_VALUE,
    });
    expect(shown[2]).toMatchObject({
      text: 'shown',
      state: 'normal',
      value: textValue('exact'),
    });
  });

  it('marks deletions and keeps display-only fields until loaded', () => {
    const deleted = withStaged(fields, { ...edit, kind: 'delete' }, null);
    expect(deleted.map((field) => field.state)).toEqual([
      'deleted',
      'deleted',
      'deleted',
    ]);
    expect(withStaged(fields, undefined, null)[0]?.value).toBeUndefined();
    expect(withStaged(fields, undefined, { id: null })[0]?.value).toEqual(
      NULL_VALUE,
    );
  });
});

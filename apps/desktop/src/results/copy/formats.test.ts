// SPDX-License-Identifier: Apache-2.0
import { describe, expect, it } from 'vitest';
import { formatTable } from './formats';
import type { CopyTable } from './formats';
import { quoteIdent, sqlLiteral, tableName, valueKind } from './sql';

const table: CopyTable = {
  columns: [
    { name: 'id', kind: 'number' },
    { name: 'note', kind: 'text' },
    { name: 'ok', kind: 'boolean' },
  ],
  rows: [
    ['1', 'plain', 'true'],
    ['2', null, 'false'],
    ['3', 'it\'s "a|b"\nnext', null],
  ],
};
const pg = { engine: 'postgres' as const, table: '"public"."t"' };

describe('copy formats', () => {
  it('writes TSV without a header and quotes only special values', () => {
    expect(formatTable('tsv', table, pg)).toBe(
      '1\tplain\ttrue\n2\tNULL\tfalse\n3\t"it\'s ""a|b""\nnext"\tNULL',
    );
  });
  it('writes RFC 4180 CSV with a header; NULL is empty', () => {
    expect(formatTable('csv', table, pg)).toBe(
      'id,note,ok\n1,plain,true\n2,,false\n3,"it\'s ""a|b""\nnext",',
    );
  });
  it('writes JSON with exact numbers, booleans and null', () => {
    const big: CopyTable = {
      columns: [
        { name: 'n', kind: 'number' },
        { name: 'd', kind: 'number' },
      ],
      rows: [
        ['9007199254740993', '.5'],
        ['NaN', '1.'],
      ],
    };
    expect(formatTable('json', big, pg)).toBe(
      '[\n  {\n    "n": 9007199254740993,\n    "d": 0.5\n  },\n  {\n    "n": "NaN",\n    "d": 1\n  }\n]',
    );
    expect(JSON.parse(formatTable('json', table, pg))).toEqual([
      { id: 1, note: 'plain', ok: true },
      { id: 2, note: null, ok: false },
      { id: 3, note: 'it\'s "a|b"\nnext', ok: null },
    ]);
    expect(formatTable('json', { ...table, rows: [] }, pg)).toBe('[]');
  });
  it('writes a Markdown table with escaped pipes and line breaks', () => {
    expect(formatTable('markdown', table, pg)).toBe(
      [
        '| id | note | ok |',
        '| ---: | --- | --- |',
        '| 1 | plain | true |',
        '| 2 | NULL | false |',
        '| 3 | it\'s "a\\|b"<br>next | NULL |',
      ].join('\n'),
    );
  });
  it('writes one INSERT per row with dialect literals', () => {
    expect(formatTable('insert', table, pg)).toBe(
      [
        `INSERT INTO "public"."t" ("id", "note", "ok") VALUES (1, 'plain', TRUE);`,
        `INSERT INTO "public"."t" ("id", "note", "ok") VALUES (2, NULL, FALSE);`,
        `INSERT INTO "public"."t" ("id", "note", "ok") VALUES (3, 'it''s "a|b"\nnext', NULL);`,
      ].join('\n'),
    );
    const mysql = { engine: 'mysql' as const, table: '`t`' };
    expect(
      formatTable('insert', { ...table, rows: [table.rows[2] ?? []] }, mysql),
    ).toBe(
      "INSERT INTO `t` (`id`, `note`, `ok`) VALUES (3, 'it\\'s \"a|b\"\\nnext', NULL);",
    );
  });
  it('writes WHERE IN over distinct values with NULL as IS NULL', () => {
    const sqlite = { engine: 'sqlite' as const, table: '"t"' };
    const rows: CopyTable = {
      columns: [
        { name: 'id', kind: 'number' },
        { name: 'ok', kind: 'boolean' },
        { name: 'gone', kind: 'text' },
      ],
      rows: [
        ['1', 'true', null],
        ['1', null, null],
        ['2', 'false', null],
      ],
    };
    expect(formatTable('whereIn', rows, sqlite)).toBe(
      'WHERE "id" IN (1, 2)\nAND ("ok" IN (1, 0) OR "ok" IS NULL)\nAND "gone" IS NULL',
    );
  });
});

describe('SQL quoting', () => {
  it('quotes identifiers per dialect', () => {
    expect(quoteIdent('postgres', 'a"b')).toBe('"a""b"');
    expect(quoteIdent('sqlite', 'x')).toBe('"x"');
    expect(quoteIdent('mariadb', 'a`b')).toBe('`a``b`');
    expect(tableName('postgres', { schema: 'public', name: 't' })).toBe(
      '"public"."t"',
    );
    expect(tableName('mysql', { schema: 'shop', name: 't' })).toBe('`t`');
    expect(tableName('sqlite', { schema: 'main', name: 't' })).toBe('"t"');
  });
  it('writes literals like the Rust exporter', () => {
    expect(sqlLiteral('postgres', "a\\b'c", 'text')).toBe("E'a\\\\b''c'");
    expect(sqlLiteral('postgres', "o'k", 'text')).toBe("'o''k'");
    expect(sqlLiteral('mysql', 'a\\b\0\r\u001a', 'text')).toBe(
      "'a\\\\b\\0\\r\\Z'",
    );
    expect(sqlLiteral('sqlite', 'a\\b', 'text')).toBe("'a\\b'");
    expect(sqlLiteral('postgres', '12.50', 'number')).toBe('12.50');
    expect(sqlLiteral('postgres', 'NaN', 'number')).toBe("'NaN'");
    expect(sqlLiteral('sqlite', 'false', 'boolean')).toBe('0');
    expect(sqlLiteral('mysql', null, 'text')).toBe('NULL');
  });
  it('derives literal kinds from types', () => {
    expect(valueKind('int4', true)).toBe('number');
    expect(valueKind('bool', false)).toBe('boolean');
    expect(valueKind('BOOLEAN', false)).toBe('boolean');
    expect(valueKind('text', false)).toBe('text');
    expect(valueKind(null, false)).toBe('text');
  });
});

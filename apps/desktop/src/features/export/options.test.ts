// SPDX-License-Identifier: Apache-2.0
import { describe, expect, it } from 'vitest';
import type { StatementDto } from '@openrows/ipc';
import { defaultExportSettings, parseExportSettings } from './exportSettings';
import {
  formatDto,
  nullTextError,
  rememberedOptions,
  sheetOptions,
  sqlError,
} from './options';
import { resultExport, tableExport } from './sources';

const options = sheetOptions(defaultExportSettings, 'users');

describe('export options', () => {
  it('maps each format to its IPC options', () => {
    expect(formatDto(options)).toEqual({
      kind: 'csv',
      delimiter: 'comma',
      options: {
        quote: 'double',
        header: true,
        nullText: '',
        lineEnding: 'crlf',
        encoding: 'utf8',
      },
    });
    expect(
      formatDto({
        ...options,
        format: 'tsv',
        tsv: { ...options.tsv, nullText: 'NULL', lineEnding: 'lf' },
      }),
    ).toMatchObject({
      kind: 'tsv',
      options: { nullText: 'NULL', lineEnding: 'lf' },
    });
    expect(
      formatDto({ ...options, format: 'json', json: { pretty: true } }),
    ).toEqual({
      kind: 'json',
      pretty: true,
    });
    expect(formatDto({ ...options, format: 'sql', table: ' copy ' })).toEqual({
      kind: 'sql',
      table: 'copy',
      rowsPerStatement: 100,
    });
    expect(formatDto({ ...options, format: 'ndjson' })).toEqual({
      kind: 'ndjson',
    });
    expect(formatDto({ ...options, format: 'markdown' })).toEqual({
      kind: 'markdown',
    });
    expect(formatDto({ ...options, format: 'xlsx' })).toEqual({
      kind: 'xlsx',
      header: true,
    });
    expect(formatDto({ ...options, format: 'parquet' })).toEqual({
      kind: 'parquet',
      compression: 'snappy',
    });
  });

  it('refuses SQL options without a table or with a bad batch size', () => {
    const sql = { ...options, format: 'sql' as const };
    expect(sqlError(sql)).toBeNull();
    expect(sqlError({ ...sql, table: '  ' })).toMatch(/table name/);
    for (const rows of ['0', '-1', '1.5', 'ten', '10001', ''])
      expect(sqlError({ ...sql, rowsText: rows }), rows).toMatch(
        /Rows per INSERT/,
      );
    expect(formatDto({ ...sql, rowsText: '0' })).toBeNull();
    expect(sqlError({ ...sql, format: 'csv', table: '' })).toBeNull();
  });
});

describe('export option checks', () => {
  it('refuses a NULL text holding the delimiter, the quote or a line break', () => {
    for (const nullText of [',', '"', 'a\nb'])
      expect(
        nullTextError({ ...options, csv: { ...options.csv, nullText } }),
      ).not.toBeNull();
    expect(
      nullTextError({ ...options, csv: { ...options.csv, nullText: 'NULL' } }),
    ).toBeNull();
    const tsv = { ...options, format: 'tsv' as const };
    expect(
      nullTextError({ ...tsv, tsv: { ...tsv.tsv, nullText: ',' } }),
    ).toBeNull();
    expect(
      formatDto({ ...tsv, tsv: { ...tsv.tsv, nullText: '\t' } }),
    ).toBeNull();
  });

  it('remembers the options per format and validates them on load', () => {
    const remembered = rememberedOptions({
      ...options,
      format: 'xlsx',
      rowsText: '250',
      xlsx: { header: false },
    });
    expect(remembered).toMatchObject({
      format: 'xlsx',
      sql: { rowsPerStatement: 250 },
      xlsx: { header: false },
    });
    expect(
      rememberedOptions({ ...options, rowsText: 'x' }).sql.rowsPerStatement,
    ).toBe(100);
    expect(
      parseExportSettings({
        format: 'pdf',
        csv: { delimiter: 'tab', encoding: 'ebcdic', nullText: 'N' },
        sql: { rowsPerStatement: 0 },
        parquet: { compression: 'none' },
      }),
    ).toEqual({
      ...defaultExportSettings,
      csv: { ...defaultExportSettings.csv, delimiter: 'tab', nullText: 'N' },
      parquet: { compression: 'none' },
    });
  });
});

describe('export sources', () => {
  const read: StatementDto = {
    index: 0,
    sql: 'SELECT * FROM users',
    kind: 'read',
    dangerous: false,
    verdict: 'allow',
  };

  it('offers selected rows, the received rows and every row of a truncated plain read', () => {
    const result = { id: 'r1', rows: 1000, truncated: true };
    const sheet = resultExport({
      sessionId: 's',
      result,
      statement: read,
      selected: [4, 2],
    });
    expect(sheet.scopes.map((scope) => scope.source)).toEqual([
      { kind: 'rows', sessionId: 's', resultId: 'r1', rows: [4, 2] },
      { kind: 'result', sessionId: 's', resultId: 'r1' },
      { kind: 'query', sessionId: 's', sql: 'SELECT * FROM users' },
    ]);
    expect(sheet.scopes[0]?.label).toBe('The 2 rows selected');
    const complete = { ...result, truncated: false };
    expect(
      resultExport({ sessionId: 's', result: complete, statement: read })
        .scopes,
    ).toHaveLength(1);
    const write = { ...read, kind: 'write' as const };
    expect(
      resultExport({ sessionId: 's', result, statement: write }).scopes,
    ).toHaveLength(1);
  });

  it('targets the whole table on its database session', () => {
    const sheet = tableExport({
      sessionId: 's',
      profileId: 'p',
      database: 'shop',
      schema: 'public',
      name: 'orders',
    });
    expect(sheet).toMatchObject({
      title: 'Export orders',
      fileName: 'orders',
      defaultTable: 'orders',
    });
    expect(sheet.scopes[0]?.source).toEqual({
      kind: 'table',
      sessionId: 's',
      target: {
        profileId: 'p',
        database: 'shop',
        schema: 'public',
        table: 'orders',
      },
    });
  });
});

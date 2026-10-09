// SPDX-License-Identifier: Apache-2.0
import { describe, expect, it } from 'vitest';
import type { StatementDto } from '@openrows/ipc';
import { defaultOptions, formatDto, sqlError } from './options';
import { resultExport, tableExport } from './sources';

describe('export options', () => {
  it('maps each format to its IPC options', () => {
    const options = defaultOptions('users');
    expect(formatDto(options)).toEqual({
      kind: 'csv',
      delimiter: 'comma',
      header: true,
    });
    expect(formatDto({ ...options, format: 'json', lines: true })).toEqual({
      kind: 'json',
      lines: true,
    });
    expect(formatDto({ ...options, format: 'sql', table: ' copy ' })).toEqual({
      kind: 'sql',
      table: 'copy',
      rowsPerStatement: 100,
    });
  });

  it('refuses SQL options without a table or with a bad batch size', () => {
    const sql = { ...defaultOptions('t'), format: 'sql' as const };
    expect(sqlError(sql)).toBeNull();
    expect(sqlError({ ...sql, table: '  ' })).toMatch(/table name/);
    for (const rows of ['0', '-1', '1.5', 'ten', '10001', ''])
      expect(sqlError({ ...sql, rowsPerStatement: rows }), rows).toMatch(
        /Rows per INSERT/,
      );
    expect(formatDto({ ...sql, rowsPerStatement: '0' })).toBeNull();
    expect(sqlError({ ...sql, format: 'csv', table: '' })).toBeNull();
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

  it('offers every row only for a truncated plain read', () => {
    const result = { id: 'r1', rows: 1000, truncated: true };
    const sheet = resultExport({ sessionId: 's', result, statement: read });
    expect(sheet.source).toEqual({
      kind: 'result',
      sessionId: 's',
      resultId: 'r1',
    });
    expect(sheet.full).toEqual({
      source: { kind: 'query', sessionId: 's', sql: 'SELECT * FROM users' },
      received: 1000,
    });
    const complete = { ...result, truncated: false };
    expect(
      resultExport({ sessionId: 's', result: complete, statement: read }).full,
    ).toBeUndefined();
    const write = { ...read, kind: 'write' as const };
    expect(
      resultExport({ sessionId: 's', result, statement: write }).full,
    ).toBeUndefined();
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
      source: {
        kind: 'table',
        sessionId: 's',
        target: {
          profileId: 'p',
          database: 'shop',
          schema: 'public',
          table: 'orders',
        },
      },
    });
  });
});

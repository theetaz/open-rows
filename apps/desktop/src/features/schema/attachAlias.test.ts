// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { profile } from '../../test/ipcStub';
import { aliasFor, attachedAliases, attachedSchema } from './attachAlias';

it('derives a safe, unique schema name from the file name', () => {
  expect(aliasFor('/data/logs.sqlite', [])).toBe('logs');
  expect(aliasFor('/data/2024 sales-report.db', [])).toBe('_2024_sales_report');
  expect(aliasFor('/data/main.db', [])).toBe('main_2');
  expect(aliasFor('/data/Logs.db', ['logs', 'LOGS_2'])).toBe('Logs_3');
  expect(aliasFor('/data/.db', [])).toBe('db');
  expect(aliasFor('/data/ü€.db', [])).toBe('attached');
});

it('finds attached schemas of a profile ignoring case', () => {
  const lite = profile('lite', {
    engine: 'sqlite',
    file: {
      path: '/data/app.db',
      memory: false,
      readOnly: false,
      create: false,
      attached: [{ alias: 'Logs', path: '/data/logs.db' }],
    },
  });
  expect(attachedAliases(lite)).toEqual(['Logs']);
  expect(attachedSchema(lite, 'logs')).toBe('Logs');
  expect(attachedSchema(lite, 'main')).toBeUndefined();
  expect(attachedAliases(profile('pg'))).toEqual([]);
});

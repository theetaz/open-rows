// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import {
  projectQueries,
  projectSnippet,
  queryKeyword,
  snippetFile,
} from './projectSql';

it('roundtrips shared snippet metadata in an SQL comment', () => {
  const snippet = {
    id: 'a',
    name: 'Find rows',
    prefix: 'find',
    scope: 'postgres' as const,
    body: 'SELECT ${1:*} FROM ${2:t};',
  };
  const sql = snippetFile(snippet);
  expect(
    projectSnippet({ path: 'snippets/find.sql', sql, snippet: true }),
  ).toEqual({ ...snippet, id: 'snippets/find.sql' });
});
it('offers plain SQL snippet files by stem and rejects malformed metadata', () => {
  expect(
    projectSnippet({
      path: 'snippets/count.sql',
      sql: 'SELECT count(*) FROM t',
      snippet: true,
    })?.prefix,
  ).toBe('count');
  expect(
    projectSnippet({
      path: 'snippets/count.sql',
      sql: '-- openrows-snippet: {bad}\nSELECT 1',
      snippet: true,
    }),
  ).toBeNull();
});
it('keeps saved-query SQL literal and filenames usable as keywords', () => {
  expect(queryKeyword('queries/reports/top users.sql')).toBe('top_users');
  expect(
    projectQueries([
      {
        path: 'queries/top.sql',
        sql: "SELECT '${1:raw}', :id;",
        snippet: false,
      },
    ]),
  ).toEqual([
    {
      name: 'queries/top.sql',
      prefix: 'top',
      body: "SELECT '${1:raw}', :id;",
      literal: true,
    },
  ]);
});

// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { findParams, paramNames } from './params';

it('finds each placeholder style, as the core names them', () => {
  const sql = 'SELECT f(:a, $2, ?) FROM t WHERE x = :a AND y = ?';
  expect(paramNames(sql, 'postgres', 'named')).toEqual([':a']);
  expect(paramNames(sql, 'postgres', 'numbered')).toEqual(['$2']);
  expect(paramNames(sql, 'mysql', 'anonymous')).toEqual(['?1', '?2']);
  expect(findParams(sql, 'postgres', 'named')).toEqual([
    { from: 9, name: ':a' },
    { from: 37, name: ':a' },
  ]);
  const lite = 'SELECT ?1, @b, $c, :d, ?';
  expect(paramNames(lite, 'sqlite', 'numbered')).toEqual(['?1']);
  expect(paramNames(lite, 'sqlite', 'named')).toEqual(['@b', '$c', ':d']);
  expect(paramNames(lite, 'sqlite', 'anonymous')).toEqual(['?1']);
});

it('ignores look-alikes in literals, comments, casts and slices', () => {
  const lookalikes = [
    "SELECT '$1 :name ?'",
    'SELECT "$1 :name ?"',
    'SELECT $$ $1 :name ? $$',
    'SELECT $body$ :name $body$',
    'SELECT 1 -- $1 :name ?',
    'SELECT /* $1 :name ? */ 1',
    "SELECT E'\\' $1 :x ?'",
    'SELECT x::int, y::text',
    'SELECT a[1:2], a[lo:hi]',
    `SELECT :'var', :"var"`,
    "SELECT data ?| array['a'], data ?& array['b']",
    'SELECT a$1 FROM t',
    'SELECT : name',
  ];
  for (const sql of lookalikes)
    for (const style of ['named', 'numbered', 'anonymous'] as const)
      expect(paramNames(sql, 'postgres', style), sql).toEqual([]);
  expect(paramNames("SELECT 'it\\'s :x' # :y", 'mysql', 'named')).toEqual([]);
});

it('treats T-SQL variables, temp tables and bracketed names as no parameters', () => {
  const sql =
    'SELECT @a, @@ROWCOUNT, [x:y], [a?] FROM #t WHERE id = :id AND n = ?';
  expect(paramNames(sql, 'sqlserver', 'named')).toEqual([':id']);
  expect(paramNames(sql, 'sqlserver', 'anonymous')).toEqual(['?1']);
});

it('ClickHouse typed parameters skip quoted and commented look-alikes', () => {
  expect(
    paramNames(
      "SELECT {n:UInt64}, {v:Nullable(String)}, '{x:Int32}', `a\\`{hidden:String}` -- {c:String}",
      'clickhouse',
      'named',
    ),
  ).toEqual(['{n:UInt64}', '{v:Nullable(String)}']);
});

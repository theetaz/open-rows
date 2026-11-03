// SPDX-License-Identifier: Apache-2.0
import type { Snippet } from './snippetStore';
import { t } from './messages';

/**
 * Snippets shipped with the app; each can be turned off in Settings ▸
 * Snippets. `${1:name}` fields are tab stops; repeated numbers are linked.
 */
export const BUILTIN_SNIPPETS: readonly Snippet[] = [
  {
    id: 'select',
    name: t('selectRows'),
    prefix: 'sel',
    body: 'SELECT ${1:*}\nFROM ${2:table}\nWHERE ${3:condition};',
    scope: 'all',
  },
  {
    id: 'select-join',
    name: t('selectJoin'),
    prefix: 'selj',
    body: 'SELECT ${1:*}\nFROM ${2:a}\nJOIN ${3:b} ON ${3:b}.${4:id} = ${2:a}.${5:b_id};',
    scope: 'all',
  },
  {
    id: 'count',
    name: t('countRows'),
    prefix: 'cnt',
    body: 'SELECT count(*) FROM ${1:table};',
    scope: 'all',
  },
  {
    id: 'insert',
    name: t('insertRow'),
    prefix: 'ins',
    body: 'INSERT INTO ${1:table} (${2:columns})\nVALUES (${3:values});',
    scope: 'all',
  },
  {
    id: 'update',
    name: t('updateRows'),
    prefix: 'upd',
    body: 'UPDATE ${1:table}\nSET ${2:column} = ${3:value}\nWHERE ${4:condition};',
    scope: 'all',
  },
  {
    id: 'delete',
    name: t('deleteRows'),
    prefix: 'del',
    body: 'DELETE FROM ${1:table}\nWHERE ${2:condition};',
    scope: 'all',
  },
  {
    id: 'pg-create-table',
    name: t('createTableIdentity'),
    prefix: 'ct',
    body: 'CREATE TABLE ${1:name} (\n\tid bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,\n\t${2:column} ${3:text}\n);',
    scope: 'postgres',
  },
  {
    id: 'pg-upsert',
    name: t('upsertConflict'),
    prefix: 'ups',
    body: 'INSERT INTO ${1:table} (${2:key}, ${3:column})\nVALUES (${4:key_value}, ${5:value})\nON CONFLICT (${2:key}) DO UPDATE SET ${3:column} = EXCLUDED.${3:column};',
    scope: 'postgres',
  },
  {
    id: 'pg-explain',
    name: t('explainAnalyze'),
    prefix: 'exp',
    body: 'EXPLAIN (ANALYZE, BUFFERS) ${1:query};',
    scope: 'postgres',
  },
  {
    id: 'pg-activity',
    name: t('activeSessions'),
    prefix: 'act',
    body: "SELECT pid, usename, state, now() - query_start AS runtime, query\nFROM pg_stat_activity\nWHERE state <> 'idle'\nORDER BY runtime DESC;",
    scope: 'postgres',
  },
  {
    id: 'mysql-create-table',
    name: t('createTableAuto'),
    prefix: 'ct',
    body: 'CREATE TABLE ${1:name} (\n\tid BIGINT AUTO_INCREMENT PRIMARY KEY,\n\t${2:column} ${3:VARCHAR(255)}\n);',
    scope: 'mysql',
  },
  {
    id: 'mysql-upsert',
    name: t('upsertDuplicateKey'),
    prefix: 'ups',
    body: 'INSERT INTO ${1:table} (${2:key}, ${3:column})\nVALUES (${4:key_value}, ${5:value})\nON DUPLICATE KEY UPDATE ${3:column} = VALUES(${3:column});',
    scope: 'mysql',
  },
  {
    id: 'mysql-explain',
    name: t('explainAnalyze'),
    prefix: 'exp',
    body: 'EXPLAIN ANALYZE ${1:query};',
    scope: 'mysql',
  },
  {
    id: 'mysql-processlist',
    name: t('runningThreads'),
    prefix: 'act',
    body: 'SHOW FULL PROCESSLIST;',
    scope: 'mysql',
  },
  {
    id: 'sqlserver-create-table',
    name: t('createTableIdentity'),
    prefix: 'ct',
    body: 'CREATE TABLE ${1:dbo}.${2:name} (\n\tid int IDENTITY(1, 1) PRIMARY KEY,\n\t${3:column} ${4:nvarchar(100)}\n);',
    scope: 'sqlserver',
  },
  {
    id: 'sqlserver-top',
    name: t('firstRows'),
    prefix: 'top',
    body: 'SELECT TOP (${1:100}) *\nFROM ${2:table};',
    scope: 'sqlserver',
  },
  {
    id: 'sqlserver-requests',
    name: t('runningRequests'),
    prefix: 'act',
    body: 'SELECT r.session_id, r.status, r.wait_type, r.start_time, t.text\nFROM sys.dm_exec_requests AS r\nCROSS APPLY sys.dm_exec_sql_text(r.sql_handle) AS t\nWHERE r.session_id <> @@SPID;',
    scope: 'sqlserver',
  },
  {
    id: 'sqlite-create-table',
    name: t('createTableRowid'),
    prefix: 'ct',
    body: 'CREATE TABLE ${1:name} (\n\tid INTEGER PRIMARY KEY,\n\t${2:column} ${3:TEXT}\n);',
    scope: 'sqlite',
  },
  {
    id: 'sqlite-upsert',
    name: t('upsertConflict'),
    prefix: 'ups',
    body: 'INSERT INTO ${1:table} (${2:key}, ${3:column})\nVALUES (${4:key_value}, ${5:value})\nON CONFLICT (${2:key}) DO UPDATE SET ${3:column} = excluded.${3:column};',
    scope: 'sqlite',
  },
  {
    id: 'sqlite-explain',
    name: t('explainQueryPlan'),
    prefix: 'exp',
    body: 'EXPLAIN QUERY PLAN ${1:query};',
    scope: 'sqlite',
  },
  {
    id: 'sqlite-tables',
    name: t('listTables'),
    prefix: 'tbls',
    body: "SELECT name FROM sqlite_schema WHERE type = 'table' ORDER BY name;",
    scope: 'sqlite',
  },
];

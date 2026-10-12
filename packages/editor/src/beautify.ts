// SPDX-License-Identifier: Apache-2.0
import {
  formatDialect,
  mariadb,
  mysql,
  postgresql,
  sqlite,
  clickhouse,
  transactsql,
} from 'sql-formatter';
import type { DialectOptions, ParamTypes } from 'sql-formatter';
import type { EditorDialect } from './dialects';
import type { FormatOptions } from './format';

const DIALECTS: Record<EditorDialect, DialectOptions> = {
  mongodb: postgresql,
  postgres: postgresql,
  mysql,
  mariadb,
  sqlserver: transactsql,
  sqlite,
  clickhouse,
};

/**
 * Placeholders the formatter keeps as single tokens, so `:id`, `$1`, `?` and
 * SQLite's `?1`, `@id`, `$id` survive formatting unchanged. A bare `?` is not
 * one in PostgreSQL, where it is also the `jsonb` key-exists operator. In
 * T-SQL `@name` is a variable, which the formatter already keeps whole.
 */
const PARAMS: Record<EditorDialect, ParamTypes> = {
  mongodb: {},
  postgres: { numbered: ['$'], named: [':'] },
  mysql: { positional: true, named: [':'] },
  mariadb: { positional: true, named: [':'] },
  clickhouse: { named: [':'] },
  sqlserver: { positional: true, named: [':'] },
  sqlite: { positional: true, numbered: ['?', '$'], named: [':', '@', '$'] },
};

/** Pretty-print SQL; loaded on first use so the formatter stays out of startup. */
export function beautify(sql: string, options: FormatOptions): string {
  if (options.dialect === 'mongodb')
    throw new Error('Edit aggregation stages as JSON in the Pipeline editor.');
  return formatDialect(sql, {
    dialect: DIALECTS[options.dialect],
    tabWidth: options.indentWidth,
    useTabs: options.indentWith === 'tabs',
    keywordCase: options.keywordCase,
    indentStyle: options.indentStyle,
    linesBetweenQueries: options.linesBetweenQueries,
    denseOperators: options.denseOperators,
    paramTypes: PARAMS[options.dialect],
  });
}

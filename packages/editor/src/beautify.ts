// SPDX-License-Identifier: Apache-2.0
import {
  formatDialect,
  mariadb,
  mysql,
  postgresql,
  sqlite,
} from 'sql-formatter';
import type { DialectOptions, ParamTypes } from 'sql-formatter';
import type { EditorDialect } from './dialects';
import type { FormatOptions } from './format';

const DIALECTS: Record<EditorDialect, DialectOptions> = {
  postgres: postgresql,
  mysql,
  mariadb,
  sqlite,
};

/**
 * Placeholders the formatter keeps as single tokens, so `:id`, `$1`, `?` and
 * SQLite's `?1`, `@id`, `$id` survive formatting unchanged. A bare `?` is not
 * one in PostgreSQL, where it is also the `jsonb` key-exists operator.
 */
const PARAMS: Record<EditorDialect, ParamTypes> = {
  postgres: { numbered: ['$'], named: [':'] },
  mysql: { positional: true, named: [':'] },
  mariadb: { positional: true, named: [':'] },
  sqlite: { positional: true, numbered: ['?', '$'], named: [':', '@', '$'] },
};

/** Pretty-print SQL; loaded on first use so the formatter stays out of startup. */
export function beautify(sql: string, options: FormatOptions): string {
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

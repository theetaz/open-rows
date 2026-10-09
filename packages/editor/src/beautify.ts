// SPDX-License-Identifier: Apache-2.0
import {
  formatDialect,
  mariadb,
  mysql,
  postgresql,
  sqlite,
} from 'sql-formatter';
import type { DialectOptions } from 'sql-formatter';
import type { EditorDialect } from './dialects';
import type { FormatOptions } from './format';

const DIALECTS: Record<EditorDialect, DialectOptions> = {
  postgres: postgresql,
  mysql,
  mariadb,
  sqlite,
};

/** Pretty-print SQL; loaded on first use so the formatter stays out of startup. */
export function beautify(sql: string, options: FormatOptions): string {
  return formatDialect(sql, {
    dialect: DIALECTS[options.dialect],
    tabWidth: options.indentWidth,
    useTabs: options.indentWith === 'tabs',
    keywordCase: options.keywordCase,
    indentStyle: options.indentStyle,
  });
}

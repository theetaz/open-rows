// SPDX-License-Identifier: Apache-2.0
import { MariaSQL, MySQL, PostgreSQL, SQLite } from '@codemirror/lang-sql';
import type { SQLDialect } from '@codemirror/lang-sql';

/** SQL dialects the editor highlights, one per connection engine. */
export type EditorDialect = 'postgres' | 'mysql' | 'mariadb' | 'sqlite';

const DIALECTS: Record<EditorDialect, SQLDialect> = {
  postgres: PostgreSQL,
  mysql: MySQL,
  mariadb: MariaSQL,
  sqlite: SQLite,
};

/** The CodeMirror dialect of an engine. */
export function sqlDialect(dialect: EditorDialect): SQLDialect {
  return DIALECTS[dialect];
}

// SPDX-License-Identifier: Apache-2.0
import {
  MSSQL,
  MariaSQL,
  MySQL,
  PostgreSQL,
  SQLite,
  SQLDialect,
} from '@codemirror/lang-sql';

/** SQL dialects the editor highlights, one per connection engine. */
export type EditorDialect =
  | 'mongodb'
  | 'postgres'
  | 'mysql'
  | 'mariadb'
  | 'sqlserver'
  | 'sqlite'
  | 'clickhouse';

const DIALECTS: Record<EditorDialect, SQLDialect> = {
  mongodb: SQLDialect.define({
    doubleQuotedStrings: true,
    backslashEscapes: true,
    keywords:
      'db find sort limit skip aggregate countDocuments insertOne insertMany updateOne updateMany deleteOne deleteMany replaceOne createIndex dropIndex getIndexes getCollection toArray',
    types: '',
  }),
  postgres: PostgreSQL,
  mysql: MySQL,
  mariadb: MariaSQL,
  sqlserver: MSSQL,
  sqlite: SQLite,
  clickhouse: SQLDialect.define({
    ...MySQL.spec,
    doubleQuotedStrings: false,
    keywords: `${MySQL.spec.keywords ?? ''} PREWHERE FINAL SAMPLE ARRAY JOIN SETTINGS FORMAT TTL OPTIMIZE SYSTEM KILL DICTIONARY MATERIALIZED ENGINE`,
    types:
      'Int8 Int16 Int32 Int64 UInt8 UInt16 UInt32 UInt64 Float32 Float64 String FixedString Nullable LowCardinality Decimal Date Date32 DateTime DateTime64 UUID IPv4 IPv6 Enum8 Enum16 Array Map Tuple',
  }),
};

/** The CodeMirror dialect of an engine. */
export function sqlDialect(dialect: EditorDialect): SQLDialect {
  return DIALECTS[dialect];
}

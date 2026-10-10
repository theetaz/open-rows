// SPDX-License-Identifier: Apache-2.0
import type {
  EngineDto,
  IdentityDto,
  IndexMethodDto,
  ObjectFeaturesDto,
  ReferentialActionDto,
} from '@openrows/ipc';

/** A type offered by the type picker, and the size it takes. */
type TypeSpec = { name: string; size?: 'length' | 'precision' };

type Option<T extends string> = { value: T; label: string };

const postgresTypes: readonly TypeSpec[] = [
  { name: 'text' },
  { name: 'varchar', size: 'length' },
  { name: 'char', size: 'length' },
  { name: 'integer' },
  { name: 'bigint' },
  { name: 'smallint' },
  { name: 'numeric', size: 'precision' },
  { name: 'real' },
  { name: 'double precision' },
  { name: 'boolean' },
  { name: 'date' },
  { name: 'time' },
  { name: 'timestamp' },
  { name: 'timestamptz' },
  { name: 'interval' },
  { name: 'uuid' },
  { name: 'json' },
  { name: 'jsonb' },
  { name: 'bytea' },
  { name: 'inet' },
  { name: 'text[]' },
];

const mysqlTypes: readonly TypeSpec[] = [
  { name: 'int' },
  { name: 'bigint' },
  { name: 'smallint' },
  { name: 'tinyint' },
  { name: 'int unsigned' },
  { name: 'bigint unsigned' },
  { name: 'decimal', size: 'precision' },
  { name: 'float' },
  { name: 'double' },
  { name: 'varchar', size: 'length' },
  { name: 'char', size: 'length' },
  { name: 'text' },
  { name: 'mediumtext' },
  { name: 'longtext' },
  { name: 'date' },
  { name: 'datetime' },
  { name: 'timestamp' },
  { name: 'time' },
  { name: 'year' },
  { name: 'json' },
  { name: 'blob' },
  { name: 'varbinary', size: 'length' },
  { name: 'bit', size: 'length' },
];

const sqliteTypes: readonly TypeSpec[] = [
  { name: 'INTEGER' },
  { name: 'TEXT' },
  { name: 'REAL' },
  { name: 'NUMERIC' },
  { name: 'BLOB' },
  { name: 'VARCHAR', size: 'length' },
  { name: 'BOOLEAN' },
  { name: 'DATE' },
  { name: 'DATETIME' },
];

/** The types the picker offers for `engine`, in picker order. */
export function typeSpecs(engine: EngineDto): readonly TypeSpec[] {
  switch (engine) {
    case 'mysql':
    case 'mariadb':
      return mysqlTypes;
    case 'sqlite':
    case 'libsql':
    case 'libsql-file':
      return sqliteTypes;
    default:
      return postgresTypes;
  }
}

const allMethods: readonly Option<IndexMethodDto>[] = [
  { value: 'btree', label: 'B-tree' },
  { value: 'hash', label: 'Hash' },
  { value: 'gin', label: 'GIN' },
  { value: 'gist', label: 'GiST' },
  { value: 'brin', label: 'BRIN' },
  { value: 'spgist', label: 'SP-GiST' },
];

/** Index access methods `engine` can create. */
export function indexMethods(
  engine: EngineDto,
): readonly Option<IndexMethodDto>[] {
  switch (engine) {
    case 'mysql':
    case 'mariadb':
      return allMethods.slice(0, 2);
    case 'sqlite':
    case 'libsql':
    case 'libsql-file':
      return allMethods.slice(0, 1);
    default:
      return allMethods;
  }
}

/** Numbering choices: identity on PostgreSQL, `AUTO_INCREMENT` on MySQL. */
export function identityOptions(
  engine: EngineDto,
  features: ObjectFeaturesDto,
): readonly Option<IdentityDto>[] {
  const none: Option<IdentityDto> = { value: 'none', label: 'None' };
  if (!features.identity) return [none];
  if (engine === 'mysql' || engine === 'mariadb')
    return [none, { value: 'autoIncrement', label: 'AUTO_INCREMENT' }];
  return [
    none,
    { value: 'always', label: 'Identity (always)' },
    { value: 'byDefault', label: 'Identity (by default)' },
  ];
}

/** Suggested collations of `engine` (any name may be typed). */
export function collationHints(engine: EngineDto): readonly string[] {
  switch (engine) {
    case 'mysql':
    case 'mariadb':
      return [
        'utf8mb4_0900_ai_ci',
        'utf8mb4_general_ci',
        'utf8mb4_bin',
        'latin1_swedish_ci',
      ];
    case 'sqlite':
    case 'libsql':
    case 'libsql-file':
      return ['BINARY', 'NOCASE', 'RTRIM'];
    default:
      return ['C', 'POSIX', 'default', 'und-x-icu', 'en-US-x-icu'];
  }
}

/** Suggested database encodings or character sets of `engine`. */
export function encodingHints(engine: EngineDto): readonly string[] {
  return engine === 'mysql' || engine === 'mariadb'
    ? ['utf8mb4', 'utf8mb3', 'latin1', 'ascii', 'binary']
    : ['UTF8', 'LATIN1', 'SQL_ASCII', 'WIN1252'];
}

/** Default presets offered by the column_default picker (SQL expressions). */
export function defaultPresets(engine: EngineDto): readonly string[] {
  switch (engine) {
    case 'mysql':
    case 'mariadb':
      return ['CURRENT_TIMESTAMP', 'uuid()', '0', "''"];
    case 'sqlite':
    case 'libsql':
    case 'libsql-file':
      return ['CURRENT_TIMESTAMP', "datetime('now')", '0', "''"];
    default:
      return [
        'now()',
        'CURRENT_TIMESTAMP',
        'gen_random_uuid()',
        'true',
        'false',
        '0',
      ];
  }
}

/** `ON DELETE` / `ON UPDATE` choices. */
export const referentialActions: readonly Option<ReferentialActionDto>[] = [
  { value: 'noAction', label: 'NO ACTION' },
  { value: 'restrict', label: 'RESTRICT' },
  { value: 'cascade', label: 'CASCADE' },
  { value: 'setNull', label: 'SET NULL' },
  { value: 'setDefault', label: 'SET DEFAULT' },
];

/** Whether `DROP … CASCADE` and `TRUNCATE … CASCADE` exist (PostgreSQL). */
export const cascades = (engine: EngineDto): boolean => engine === 'postgres';

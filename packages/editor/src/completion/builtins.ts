// SPDX-License-Identifier: Apache-2.0
import { sqlDialect } from '../dialects';
import type { EditorDialect } from '../dialects';

/** Keywords typed most often, most common first (`sel` → SELECT, `fr` → FROM). */
const COMMON_KEYWORDS = [
  'select',
  'from',
  'where',
  'join',
  'and',
  'or',
  'order',
  'group',
  'by',
  'as',
  'on',
  'insert',
  'into',
  'values',
  'update',
  'set',
  'delete',
  'left',
  'inner',
  'limit',
  'having',
  'distinct',
  'not',
  'null',
  'is',
  'in',
  'like',
  'between',
  'case',
  'when',
  'then',
  'else',
  'end',
  'create',
  'table',
  'alter',
  'drop',
  'with',
  'union',
  'exists',
  'returning',
  'offset',
  'asc',
  'desc',
];

const keywordCache = new Map<EditorDialect, readonly string[]>();

/** The dialect's keywords and type names, lower case, without duplicates. */
export function dialectKeywords(dialect: EditorDialect): readonly string[] {
  const cached = keywordCache.get(dialect);
  if (cached) return cached;
  const { spec } = sqlDialect(dialect);
  const words = `${COMMON_KEYWORDS.join(' ')} ${spec.keywords ?? ''} ${spec.types ?? ''}`;
  const list = [...new Set(words.toLowerCase().split(/\s+/).filter(Boolean))];
  keywordCache.set(dialect, list);
  return list;
}

/** Sort weight of a keyword: its place among common keywords, else last. */
export function keywordPriority(keyword: string): number {
  const index = COMMON_KEYWORDS.indexOf(keyword);
  return index < 0 ? COMMON_KEYWORDS.length : index;
}

const AGGREGATES =
  'count sum avg min max coalesce nullif abs round lower upper';
const WINDOWS =
  'row_number rank dense_rank ntile lag lead first_value last_value';

/**
 * Built-in functions offered without a catalog read: PostgreSQL's common
 * `pg_catalog` functions (the catalog index lists user schemas only), and
 * MySQL/MariaDB and SQLite built-ins, which have no function catalog.
 */
const BUILT_INS: Record<EditorDialect, string> = {
  postgres: `${AGGREGATES} ${WINDOWS} now age date_trunc date_part to_char
    to_date to_timestamp make_date make_interval clock_timestamp concat
    concat_ws string_agg array_agg json_agg jsonb_agg json_build_object
    jsonb_build_object to_json to_jsonb jsonb_set generate_series greatest
    least length char_length substr substring replace trim btrim lpad rpad
    left right position split_part regexp_replace regexp_matches initcap
    format md5 gen_random_uuid random floor ceil trunc sqrt power mod sign
    unnest array_length array_to_string string_to_array bool_and bool_or
    stddev variance percentile_cont nextval currval setval pg_typeof
    pg_size_pretty pg_total_relation_size encode decode`,
  mysql: `${AGGREGATES} ${WINDOWS} now curdate curtime date_format date_add
    date_sub datediff timestampdiff str_to_date unix_timestamp from_unixtime
    year month day hour minute second week dayofweek concat concat_ws
    group_concat ifnull if json_extract json_object json_array json_unquote
    substring substring_index locate instr length char_length replace trim
    lpad rpad left right floor ceil ceiling rand md5 sha1 sha2 uuid greatest
    least last_insert_id found_rows database version format truncate mod
    power sqrt regexp_replace convert`,
  mariadb: '',
  sqlserver: `${AGGREGATES} ${WINDOWS} getdate getutcdate sysdatetime
    sysutcdatetime sysdatetimeoffset dateadd datediff datediff_big datename
    datepart datefromparts datetime2fromparts eomonth isdate year month day
    format convert cast try_convert try_cast parse try_parse isnull coalesce
    nullif iif choose len datalength charindex patindex substring left right
    ltrim rtrim trim upper lower replace replicate reverse stuff concat
    concat_ws string_agg string_split quotename newid newsequentialid abs
    ceiling floor round power sqrt rand sign json_value json_query json_modify
    isjson openjson object_id object_name schema_name db_name db_id
    suser_sname user_name scope_identity ident_current row_number rank
    dense_rank ntile count_big checksum hashbytes error_message error_number`,
  sqlite: `${AGGREGATES} ${WINDOWS} total group_concat string_agg changes char
    concat concat_ws format glob hex ifnull iif instr last_insert_rowid length
    like likelihood ltrim rtrim trim printf quote random randomblob replace
    sign soundex sqlite_version substr substring total_changes typeof unhex
    unicode zeroblob date time datetime julianday unixepoch strftime timediff
    json json_array json_extract json_object json_insert json_replace json_set
    json_remove json_type json_valid json_group_array json_group_object
    json_each json_tree nth_value percent_rank cume_dist`,
};

/** Built-in function names of the dialect (MariaDB shares MySQL's). */
export function builtInFunctions(dialect: EditorDialect): readonly string[] {
  const source = dialect === 'mariadb' ? BUILT_INS.mysql : BUILT_INS[dialect];
  return [...new Set(source.split(/\s+/).filter(Boolean))];
}

/** Detail shown next to built-in functions. */
export function builtInDetail(dialect: EditorDialect): string {
  return dialect === 'postgres' ? 'pg_catalog' : 'built-in';
}

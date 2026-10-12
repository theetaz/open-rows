// SPDX-License-Identifier: Apache-2.0
//! Catalog SQL over `DuckDB`'s `duckdb_*` table functions, scoped to the opened
//! database; every column is cast to VARCHAR for the text reader.

/// Non-internal databases; the opened one is reported as `main`.
pub(super) const DATABASES: &str = "SELECT database_name, \
     (database_name = current_database())::VARCHAR, readonly::VARCHAR \
     FROM duckdb_databases() WHERE NOT internal \
     ORDER BY database_name <> current_database(), database_name";

/// Schemas of the opened database, user schemas first.
pub(super) const SCHEMAS: &str = "SELECT schema_name, internal::VARCHAR, comment \
     FROM duckdb_schemas() WHERE database_name = current_database() \
     ORDER BY internal, schema_name";

/// Tables, views, sequences and macros of one schema (`$1`), or of every
/// schema when `$1` is NULL: kind, schema, name, arguments, estimate, comment.
pub(super) const OBJECTS: &str = "SELECT kind, schema_name, name, arguments, estimate, comment FROM (\
     SELECT 'table' AS kind, schema_name, table_name AS name, NULL AS arguments, \
     estimated_size::VARCHAR AS estimate, comment FROM duckdb_tables() \
     WHERE database_name = current_database() AND NOT internal \
     UNION ALL SELECT 'view', schema_name, view_name, NULL, NULL, comment FROM duckdb_views() \
     WHERE database_name = current_database() AND NOT internal \
     UNION ALL SELECT 'sequence', schema_name, sequence_name, NULL, NULL, comment \
     FROM duckdb_sequences() WHERE database_name = current_database() \
     UNION ALL SELECT DISTINCT 'function', schema_name, function_name, \
     array_to_string(parameters, ', '), NULL, comment FROM duckdb_functions() \
     WHERE database_name = current_database() AND NOT internal \
     AND function_type IN ('macro', 'table_macro')) \
     WHERE schema_name = coalesce($1::VARCHAR, schema_name) \
     ORDER BY name, kind, arguments";

/// Columns of `$1`.`$2` in ordinal order, with primary-key membership.
pub(super) const COLUMNS: &str = "SELECT c.column_name, c.data_type, c.is_nullable::VARCHAR, \
     c.column_default, c.comment, EXISTS (SELECT 1 FROM duckdb_constraints() k \
     WHERE k.database_name = c.database_name AND k.schema_name = c.schema_name \
     AND k.table_name = c.table_name AND k.constraint_type = 'PRIMARY KEY' \
     AND list_contains(k.constraint_column_names, c.column_name))::VARCHAR \
     FROM duckdb_columns() c WHERE c.database_name = current_database() \
     AND c.schema_name = $1 AND c.table_name = $2 ORDER BY c.column_index";

/// Primary-key and unique constraint columns of `$1`.`$2`, primary first,
/// one row per column in key order.
pub(super) const KEYS: &str = "SELECT position_key, constraint_type, name FROM (\
     SELECT constraint_index::VARCHAR AS position_key, constraint_index, constraint_type, \
     unnest(constraint_column_names) AS name, \
     unnest(range(len(constraint_column_names))) AS position FROM duckdb_constraints() \
     WHERE database_name = current_database() AND schema_name = $1 AND table_name = $2 \
     AND constraint_type IN ('PRIMARY KEY', 'UNIQUE')) \
     ORDER BY constraint_type <> 'PRIMARY KEY', constraint_index, position";

/// The relation `$1`.`$2`: kind, row estimate, comment and stored SQL.
pub(super) const RELATION: &str = "SELECT 'table', estimated_size::VARCHAR, comment, sql \
     FROM duckdb_tables() WHERE database_name = current_database() \
     AND schema_name = $1 AND table_name = $2 \
     UNION ALL SELECT 'view', NULL, comment, sql FROM duckdb_views() \
     WHERE database_name = current_database() AND NOT internal \
     AND schema_name = $1 AND view_name = $2";

/// Constraints of `$1`.`$2` except NOT NULL, in definition order.
pub(super) const CONSTRAINTS: &str = "SELECT constraint_name, constraint_type, constraint_text, \
     array_to_string(constraint_column_names, ', '), referenced_table, \
     array_to_string(referenced_column_names, ', ') FROM duckdb_constraints() \
     WHERE database_name = current_database() AND schema_name = $1 AND table_name = $2 \
     AND constraint_type <> 'NOT NULL' ORDER BY constraint_index";

/// Explicit indexes of `$1`.`$2`.
pub(super) const INDEXES: &str = "SELECT index_name, is_unique::VARCHAR, is_primary::VARCHAR, \
     trim(expressions, '[]'), comment, sql FROM duckdb_indexes() \
     WHERE database_name = current_database() AND schema_name = $1 AND table_name = $2 \
     ORDER BY index_name";

/// One sequence; reading it never advances it.
pub(super) const SEQUENCE: &str = "SELECT start_value::VARCHAR, min_value::VARCHAR, \
     max_value::VARCHAR, increment_by::VARCHAR, cycle::VARCHAR, last_value::VARCHAR, \
     comment, sql FROM duckdb_sequences() WHERE database_name = current_database() \
     AND schema_name = $1 AND sequence_name = $2";

/// Overloads of one macro.
pub(super) const MACRO: &str = "SELECT function_type, array_to_string(parameters, ', '), \
     return_type, comment, macro_definition FROM duckdb_functions() \
     WHERE database_name = current_database() AND NOT internal \
     AND function_type IN ('macro', 'table_macro') AND schema_name = $1 AND function_name = $2 \
     ORDER BY 2";

/// Digest of every listed object's identity and stored SQL; data and
/// sequence values do not change it.
pub(super) const FINGERPRINT: &str = "SELECT md5(coalesce(string_agg(item, chr(10) ORDER BY item), '')) FROM (\
     SELECT 'n|' || schema_name AS item FROM duckdb_schemas() WHERE database_name = current_database() \
     UNION ALL SELECT 't|' || schema_name || '|' || table_name || '|' || coalesce(sql, '') \
     FROM duckdb_tables() WHERE database_name = current_database() AND NOT internal \
     UNION ALL SELECT 'v|' || schema_name || '|' || view_name || '|' || coalesce(sql, '') \
     FROM duckdb_views() WHERE database_name = current_database() AND NOT internal \
     UNION ALL SELECT 's|' || schema_name || '|' || sequence_name FROM duckdb_sequences() \
     WHERE database_name = current_database() \
     UNION ALL SELECT 'i|' || schema_name || '|' || index_name || '|' || coalesce(sql, '') \
     FROM duckdb_indexes() WHERE database_name = current_database() \
     UNION ALL SELECT 'f|' || schema_name || '|' || function_name || '|' || coalesce(macro_definition, '') \
     FROM duckdb_functions() WHERE database_name = current_database() AND NOT internal)";

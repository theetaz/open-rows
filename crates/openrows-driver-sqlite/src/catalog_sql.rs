// SPDX-License-Identifier: Apache-2.0
//! `SQLite` catalog queries shared by the local `SQLite` and remote Hrana paths.
/// Attached database names in attachment order.
pub const SCHEMAS: &str = include_str!("../sql/schemas.sql");
/// Visible and generated columns; parameters are table, schema.
pub const COLUMNS: &str = include_str!("../sql/columns.sql");
/// Index flags; parameters are table, schema.
pub const INDEXES: &str = include_str!("../sql/indexes.sql");
/// Index columns in key order; parameters are index, schema.
pub const INDEX_COLUMNS: &str = include_str!("../sql/index_columns.sql");
/// Foreign-key columns in constraint order; parameters are table, schema.
pub const FOREIGN_KEYS: &str = include_str!("../sql/foreign_keys.sql");
/// Rowid eligibility; parameters are schema, table.
pub const ROWID_TABLE: &str = include_str!("../sql/rowid_table.sql");

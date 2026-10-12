// SPDX-License-Identifier: Apache-2.0
//! Input and output of structure-change generation (docs/10 §3).
use crate::schema::TableDetail;

/// The table a change set applies to, with its current catalog detail.
#[derive(Clone, Copy, Debug)]
pub struct AlterTable<'a> {
    /// Schema name.
    pub schema: &'a str,
    /// Current table name.
    pub table: &'a str,
    /// Catalog detail read just before generating.
    pub detail: &'a TableDetail,
}

/// How the generated DDL executes on a fresh connection.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AlterRun {
    /// The engine's DDL editor (transactional where supported).
    #[default]
    Transaction,
    /// Direct execution, outside a transaction (`PostgreSQL` concurrent indexes).
    Autocommit,
    /// `MySQL`/`MariaDB` DDL after verifying session strict mode, preventing NULL coercion.
    StrictMySql,
}

/// Ordered statements, user-facing warnings and whether data can be lost.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AlterPlan {
    /// Transaction boundary and execution preconditions.
    pub run: AlterRun,
    /// Ordered SQL without trailing `;`; [`execute_plan`] honours [`Self::run`].
    pub statements: Vec<String>,
    /// Lock, rewrite and failure warnings for the review sheet.
    pub warnings: Vec<String>,
    /// Drops a column or index, or changes a column type.
    pub destructive: bool,
}

mod narrowing;
mod options;
mod strict_mode;
pub use options::{execute_plan, key_sql, validate_options};
pub(crate) mod staged;

pub use narrowing::{narrowing_warning, narrows};
pub use staged::{AlterError, Collation, ColumnEdits, Comment, IndexDrops, StagedChanges};

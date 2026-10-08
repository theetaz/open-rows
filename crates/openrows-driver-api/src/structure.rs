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

/// Ordered statements, user-facing warnings and whether data can be lost.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AlterPlan {
    /// Statements to run in one transaction, without trailing `;`.
    pub statements: Vec<String>,
    /// Lock, rewrite and failure warnings for the review sheet.
    pub warnings: Vec<String>,
    /// Drops a column or index, or changes a column type.
    pub destructive: bool,
}

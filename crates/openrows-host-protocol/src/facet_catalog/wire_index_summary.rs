// SPDX-License-Identifier: Apache-2.0
//! Dedicated bounded facet DTOs; native API layouts never cross the transport.
use serde::{Deserialize, Serialize};
/// Transport representation of `IndexSummary`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WireIndexSummary {
    /// Index name.
    pub name: String,
    /// Key columns or expressions, comma-separated, in key order.
    pub columns: String,
    /// Enforces uniqueness.
    pub unique: bool,
    /// Backs the primary key.
    pub primary: bool,
    /// Access method, e.g. `btree` or `gin`.
    pub method: String,
    /// Partial-index predicate, when set.
    pub predicate: Option<String>,
}
impl From<openrows_driver_api::catalog::IndexSummary> for WireIndexSummary {
    fn from(value: openrows_driver_api::catalog::IndexSummary) -> Self {
        Self {
            name: value.name,
            columns: value.columns,
            unique: value.unique,
            primary: value.primary,
            method: value.method,
            predicate: value.predicate,
        }
    }
}
impl From<WireIndexSummary> for openrows_driver_api::catalog::IndexSummary {
    fn from(value: WireIndexSummary) -> Self {
        Self {
            name: value.name,
            columns: value.columns,
            unique: value.unique,
            primary: value.primary,
            method: value.method,
            predicate: value.predicate,
        }
    }
}

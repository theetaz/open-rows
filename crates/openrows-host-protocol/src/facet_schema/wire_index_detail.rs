// SPDX-License-Identifier: Apache-2.0
//! Dedicated bounded facet DTOs; native API layouts never cross the transport.
use serde::{Deserialize, Serialize};
/// Transport representation of `IndexDetail`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WireIndexDetail {
    /// Index name.
    pub name: String,
    /// Access method, e.g. `btree`.
    pub method: String,
    /// Enforces uniqueness.
    pub unique: bool,
    /// Backs the primary key.
    pub primary: bool,
    /// Backs a constraint (primary key, unique or exclusion).
    pub constraint: bool,
    /// Key columns or expressions, comma-separated.
    pub columns: String,
    /// `INCLUDE` columns.
    #[serde(deserialize_with = "crate::bounded::list")]
    pub include: Vec<String>,
    /// Partial-index predicate.
    pub predicate: Option<String>,
    /// Comment, when set.
    pub comment: Option<String>,
    /// `CREATE INDEX …` as the server prints it.
    pub definition: String,
}
impl From<openrows_driver_api::schema::IndexDetail> for WireIndexDetail {
    fn from(value: openrows_driver_api::schema::IndexDetail) -> Self {
        Self {
            name: value.name,
            method: value.method,
            unique: value.unique,
            primary: value.primary,
            constraint: value.constraint,
            columns: value.columns,
            include: value.include,
            predicate: value.predicate,
            comment: value.comment,
            definition: value.definition,
        }
    }
}
impl From<WireIndexDetail> for openrows_driver_api::schema::IndexDetail {
    fn from(value: WireIndexDetail) -> Self {
        Self {
            name: value.name,
            method: value.method,
            unique: value.unique,
            primary: value.primary,
            constraint: value.constraint,
            columns: value.columns,
            include: value.include,
            predicate: value.predicate,
            comment: value.comment,
            definition: value.definition,
        }
    }
}

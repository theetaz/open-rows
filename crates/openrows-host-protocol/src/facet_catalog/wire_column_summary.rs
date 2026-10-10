// SPDX-License-Identifier: Apache-2.0
//! Dedicated bounded facet DTOs; native API layouts never cross the transport.
use serde::{Deserialize, Serialize};
/// Transport representation of `ColumnSummary`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WireColumnSummary {
    /// Column name.
    pub name: String,
    /// Formatted type, e.g. `character varying(40)` or `timestamp with time zone`.
    pub data_type: String,
    /// `false` for `NOT NULL` columns.
    pub nullable: bool,
    /// Default or generation expression, when set.
    pub default: Option<String>,
    /// Comment, when set.
    pub comment: Option<String>,
    /// Part of the primary key.
    pub primary_key: bool,
    /// Labels in sort order when the column's type (or its domain's base type) is an enum.
    pub enum_labels: Option<Vec<String>>,
}
impl From<openrows_driver_api::catalog::ColumnSummary> for WireColumnSummary {
    fn from(value: openrows_driver_api::catalog::ColumnSummary) -> Self {
        Self {
            name: value.name,
            data_type: value.data_type,
            nullable: value.nullable,
            default: value.default,
            comment: value.comment,
            primary_key: value.primary_key,
            enum_labels: value.enum_labels,
        }
    }
}
impl From<WireColumnSummary> for openrows_driver_api::catalog::ColumnSummary {
    fn from(value: WireColumnSummary) -> Self {
        Self {
            name: value.name,
            data_type: value.data_type,
            nullable: value.nullable,
            default: value.default,
            comment: value.comment,
            primary_key: value.primary_key,
            enum_labels: value.enum_labels,
        }
    }
}

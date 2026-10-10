// SPDX-License-Identifier: Apache-2.0
//! Dedicated bounded facet DTOs; native API layouts never cross the transport.
use crate::{WireColumnSummary, WireIndexSummary};
use serde::{Deserialize, Serialize};
/// Transport representation of `TableStructure`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WireTableStructure {
    /// Columns in ordinal order; empty when the relation does not exist.
    #[serde(deserialize_with = "crate::bounded::list")]
    pub columns: Vec<WireColumnSummary>,
    /// Indexes ordered by name.
    #[serde(deserialize_with = "crate::bounded::list")]
    pub indexes: Vec<WireIndexSummary>,
    /// Planner estimate (`reltuples`); `None` when never analysed or not a table.
    pub row_estimate: Option<u64>,
}
impl From<openrows_driver_api::catalog::TableStructure> for WireTableStructure {
    fn from(value: openrows_driver_api::catalog::TableStructure) -> Self {
        Self {
            columns: value.columns.into_iter().map(Into::into).collect(),
            indexes: value.indexes.into_iter().map(Into::into).collect(),
            row_estimate: value.row_estimate,
        }
    }
}
impl From<WireTableStructure> for openrows_driver_api::catalog::TableStructure {
    fn from(value: WireTableStructure) -> Self {
        Self {
            columns: value.columns.into_iter().map(Into::into).collect(),
            indexes: value.indexes.into_iter().map(Into::into).collect(),
            row_estimate: value.row_estimate,
        }
    }
}

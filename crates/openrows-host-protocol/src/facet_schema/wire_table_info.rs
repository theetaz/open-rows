// SPDX-License-Identifier: Apache-2.0
//! Dedicated bounded facet DTOs; native API layouts never cross the transport.
use crate::WireRelationKind;
use serde::{Deserialize, Serialize};
/// Transport representation of `TableInfo`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WireTableInfo {
    /// Relation kind.
    pub kind: WireRelationKind,
    /// Planner estimate; `None` when never analysed.
    pub row_estimate: Option<u64>,
    /// Table, TOAST and indexes (`pg_total_relation_size`).
    pub total_bytes: u64,
    /// Main fork (`pg_relation_size`).
    pub data_bytes: u64,
    /// All indexes (`pg_indexes_size`).
    pub index_bytes: u64,
    /// Comment, when set.
    pub comment: Option<String>,
    /// View query of a view or materialized view.
    pub view_definition: Option<String>,
    /// `PARTITION BY` clause body of a partitioned table, e.g. `RANGE (day)`.
    pub partition_key: Option<String>,
    /// The server's own `CREATE` statement (`MySQL` `SHOW CREATE TABLE`,
    /// `SQLite` `sqlite_schema.sql`); `None` when it must be rebuilt.
    pub definition: Option<String>,
}
impl From<openrows_driver_api::schema::TableInfo> for WireTableInfo {
    fn from(value: openrows_driver_api::schema::TableInfo) -> Self {
        Self {
            kind: value.kind.into(),
            row_estimate: value.row_estimate,
            total_bytes: value.total_bytes,
            data_bytes: value.data_bytes,
            index_bytes: value.index_bytes,
            comment: value.comment,
            view_definition: value.view_definition,
            partition_key: value.partition_key,
            definition: value.definition,
        }
    }
}
impl From<WireTableInfo> for openrows_driver_api::schema::TableInfo {
    fn from(value: WireTableInfo) -> Self {
        Self {
            kind: value.kind.into(),
            row_estimate: value.row_estimate,
            total_bytes: value.total_bytes,
            data_bytes: value.data_bytes,
            index_bytes: value.index_bytes,
            comment: value.comment,
            view_definition: value.view_definition,
            partition_key: value.partition_key,
            definition: value.definition,
        }
    }
}

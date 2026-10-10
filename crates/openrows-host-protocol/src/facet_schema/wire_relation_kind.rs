// SPDX-License-Identifier: Apache-2.0
//! Dedicated bounded facet DTOs; native API layouts never cross the transport.
use serde::{Deserialize, Serialize};
/// Transport representation of `RelationKind`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum WireRelationKind {
    /// Ordinary table.
    Table,
    /// Partitioned table.
    PartitionedTable,
    /// View.
    View,
    /// Materialized view.
    MaterializedView,
    /// Foreign table.
    ForeignTable,
}
impl From<openrows_driver_api::schema::RelationKind> for WireRelationKind {
    fn from(value: openrows_driver_api::schema::RelationKind) -> Self {
        match value {
            openrows_driver_api::schema::RelationKind::Table => Self::Table,
            openrows_driver_api::schema::RelationKind::PartitionedTable => Self::PartitionedTable,
            openrows_driver_api::schema::RelationKind::View => Self::View,
            openrows_driver_api::schema::RelationKind::MaterializedView => Self::MaterializedView,
            openrows_driver_api::schema::RelationKind::ForeignTable => Self::ForeignTable,
        }
    }
}
impl From<WireRelationKind> for openrows_driver_api::schema::RelationKind {
    fn from(value: WireRelationKind) -> Self {
        match value {
            WireRelationKind::Table => Self::Table,
            WireRelationKind::PartitionedTable => Self::PartitionedTable,
            WireRelationKind::View => Self::View,
            WireRelationKind::MaterializedView => Self::MaterializedView,
            WireRelationKind::ForeignTable => Self::ForeignTable,
        }
    }
}

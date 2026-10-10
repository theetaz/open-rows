// SPDX-License-Identifier: Apache-2.0
//! Dedicated bounded facet DTOs; native API layouts never cross the transport.
use crate::{
    WireColumnDetail, WireConstraintDetail, WireIndexDetail, WirePartitionDetail, WireTableInfo,
    WireTriggerDetail,
};
use serde::{Deserialize, Serialize};
/// Transport representation of `TableDetail`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WireTableDetail {
    /// Columns in ordinal order.
    #[serde(deserialize_with = "crate::bounded::list")]
    pub columns: Vec<WireColumnDetail>,
    /// Constraints: primary key, unique, check, foreign key, exclusion.
    #[serde(deserialize_with = "crate::bounded::list")]
    pub constraints: Vec<WireConstraintDetail>,
    /// Indexes ordered by name.
    #[serde(deserialize_with = "crate::bounded::list")]
    pub indexes: Vec<WireIndexDetail>,
    /// User triggers ordered by name.
    #[serde(deserialize_with = "crate::bounded::list")]
    pub triggers: Vec<WireTriggerDetail>,
    /// Partitions of a partitioned table, ordered by name.
    #[serde(deserialize_with = "crate::bounded::list")]
    pub partitions: Vec<WirePartitionDetail>,
    /// `None` when the relation does not exist.
    pub info: Option<WireTableInfo>,
}
impl From<openrows_driver_api::schema::TableDetail> for WireTableDetail {
    fn from(value: openrows_driver_api::schema::TableDetail) -> Self {
        Self {
            columns: value.columns.into_iter().map(Into::into).collect(),
            constraints: value.constraints.into_iter().map(Into::into).collect(),
            indexes: value.indexes.into_iter().map(Into::into).collect(),
            triggers: value.triggers.into_iter().map(Into::into).collect(),
            partitions: value.partitions.into_iter().map(Into::into).collect(),
            info: value.info.map(Into::into),
        }
    }
}
impl From<WireTableDetail> for openrows_driver_api::schema::TableDetail {
    fn from(value: WireTableDetail) -> Self {
        Self {
            columns: value.columns.into_iter().map(Into::into).collect(),
            constraints: value.constraints.into_iter().map(Into::into).collect(),
            indexes: value.indexes.into_iter().map(Into::into).collect(),
            triggers: value.triggers.into_iter().map(Into::into).collect(),
            partitions: value.partitions.into_iter().map(Into::into).collect(),
            info: value.info.map(Into::into),
        }
    }
}

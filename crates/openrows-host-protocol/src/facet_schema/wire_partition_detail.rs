// SPDX-License-Identifier: Apache-2.0
//! Dedicated bounded facet DTOs; native API layouts never cross the transport.
use serde::{Deserialize, Serialize};
/// Transport representation of `PartitionDetail`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WirePartitionDetail {
    /// Schema of the partition.
    pub schema: String,
    /// Partition (table) name.
    pub name: String,
    /// The bound as the server prints it, e.g. `FOR VALUES FROM ('2024-01-01') TO ('2024-02-01')`.
    pub bound: String,
    /// The partition is itself partitioned.
    pub partitioned: bool,
    /// Planner estimate; `None` when never analysed.
    pub row_estimate: Option<u64>,
}
impl From<openrows_driver_api::schema::PartitionDetail> for WirePartitionDetail {
    fn from(value: openrows_driver_api::schema::PartitionDetail) -> Self {
        Self {
            schema: value.schema,
            name: value.name,
            bound: value.bound,
            partitioned: value.partitioned,
            row_estimate: value.row_estimate,
        }
    }
}
impl From<WirePartitionDetail> for openrows_driver_api::schema::PartitionDetail {
    fn from(value: WirePartitionDetail) -> Self {
        Self {
            schema: value.schema,
            name: value.name,
            bound: value.bound,
            partitioned: value.partitioned,
            row_estimate: value.row_estimate,
        }
    }
}

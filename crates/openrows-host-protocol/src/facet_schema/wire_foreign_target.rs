// SPDX-License-Identifier: Apache-2.0
//! Dedicated bounded facet DTOs; native API layouts never cross the transport.
use serde::{Deserialize, Serialize};
/// Transport representation of `ForeignTarget`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WireForeignTarget {
    /// Referenced schema.
    pub schema: String,
    /// Referenced table.
    pub table: String,
    /// Referenced columns, in key order.
    #[serde(deserialize_with = "crate::bounded::list")]
    pub columns: Vec<String>,
}
impl From<openrows_driver_api::schema::ForeignTarget> for WireForeignTarget {
    fn from(value: openrows_driver_api::schema::ForeignTarget) -> Self {
        Self {
            schema: value.schema,
            table: value.table,
            columns: value.columns,
        }
    }
}
impl From<WireForeignTarget> for openrows_driver_api::schema::ForeignTarget {
    fn from(value: WireForeignTarget) -> Self {
        Self {
            schema: value.schema,
            table: value.table,
            columns: value.columns,
        }
    }
}

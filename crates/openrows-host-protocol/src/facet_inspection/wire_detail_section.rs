// SPDX-License-Identifier: Apache-2.0
//! Dedicated bounded facet DTOs; native API layouts never cross the transport.
use serde::{Deserialize, Serialize};
/// Transport representation of `DetailSection`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WireDetailSection {
    /// Section label, such as Arguments or Enum labels.
    pub name: String,
    /// Column labels in row order.
    #[serde(deserialize_with = "crate::bounded::list")]
    pub columns: Vec<String>,
    /// Text cells, with SQL NULL kept distinct from empty text.
    #[serde(deserialize_with = "crate::bounded::rows")]
    pub rows: Vec<Vec<Option<String>>>,
}
impl From<openrows_driver_api::inspection::DetailSection> for WireDetailSection {
    fn from(value: openrows_driver_api::inspection::DetailSection) -> Self {
        Self {
            name: value.name,
            columns: value.columns,
            rows: value.rows,
        }
    }
}
impl From<WireDetailSection> for openrows_driver_api::inspection::DetailSection {
    fn from(value: WireDetailSection) -> Self {
        Self {
            name: value.name,
            columns: value.columns,
            rows: value.rows,
        }
    }
}

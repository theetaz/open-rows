// SPDX-License-Identifier: Apache-2.0
//! Dedicated bounded facet DTOs; native API layouts never cross the transport.
use serde::{Deserialize, Serialize};
/// Transport representation of `Attribute`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WireAttribute {
    /// Human-readable attribute label.
    pub name: String,
    /// Server value; NULL means unavailable, never a fabricated default.
    pub value: Option<String>,
}
impl From<openrows_driver_api::inspection::Attribute> for WireAttribute {
    fn from(value: openrows_driver_api::inspection::Attribute) -> Self {
        Self {
            name: value.name,
            value: value.value,
        }
    }
}
impl From<WireAttribute> for openrows_driver_api::inspection::Attribute {
    fn from(value: WireAttribute) -> Self {
        Self {
            name: value.name,
            value: value.value,
        }
    }
}

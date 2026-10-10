// SPDX-License-Identifier: Apache-2.0
//! Dedicated bounded facet DTOs; native API layouts never cross the transport.
use crate::{WireAttribute, WireDetailSection};
use serde::{Deserialize, Serialize};
/// Transport representation of `ObjectDetail`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WireObjectDetail {
    /// Attributes, in display order.
    #[serde(deserialize_with = "crate::bounded::list")]
    pub attributes: Vec<WireAttribute>,
    /// Columns, arguments, fields, labels or constraints.
    #[serde(deserialize_with = "crate::bounded::list")]
    pub sections: Vec<WireDetailSection>,
    /// Definition when the server exposes one to the current user.
    pub definition: Option<String>,
}
impl From<openrows_driver_api::inspection::ObjectDetail> for WireObjectDetail {
    fn from(value: openrows_driver_api::inspection::ObjectDetail) -> Self {
        Self {
            attributes: value.attributes.into_iter().map(Into::into).collect(),
            sections: value.sections.into_iter().map(Into::into).collect(),
            definition: value.definition,
        }
    }
}
impl From<WireObjectDetail> for openrows_driver_api::inspection::ObjectDetail {
    fn from(value: WireObjectDetail) -> Self {
        Self {
            attributes: value.attributes.into_iter().map(Into::into).collect(),
            sections: value.sections.into_iter().map(Into::into).collect(),
            definition: value.definition,
        }
    }
}

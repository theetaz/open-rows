// SPDX-License-Identifier: Apache-2.0
//! Dedicated bounded facet DTOs; native API layouts never cross the transport.
use crate::WireObjectKind;
use serde::{Deserialize, Serialize};
/// Transport representation of `ObjectSummary`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WireObjectSummary {
    /// Owning schema.
    pub schema: String,
    /// Object name (without arguments).
    pub name: String,
    /// Object kind.
    pub kind: WireObjectKind,
    /// Identity arguments of a routine, e.g. `integer, text`; `None` otherwise.
    pub arguments: Option<String>,
    /// Planner row estimate (not an exact count); `None` when unknown.
    pub row_estimate: Option<u64>,
    /// Comment, when set.
    pub comment: Option<String>,
}
impl From<openrows_driver_api::catalog::ObjectSummary> for WireObjectSummary {
    fn from(value: openrows_driver_api::catalog::ObjectSummary) -> Self {
        Self {
            schema: value.schema,
            name: value.name,
            kind: value.kind.into(),
            arguments: value.arguments,
            row_estimate: value.row_estimate,
            comment: value.comment,
        }
    }
}
impl From<WireObjectSummary> for openrows_driver_api::catalog::ObjectSummary {
    fn from(value: WireObjectSummary) -> Self {
        Self {
            schema: value.schema,
            name: value.name,
            kind: value.kind.into(),
            arguments: value.arguments,
            row_estimate: value.row_estimate,
            comment: value.comment,
        }
    }
}

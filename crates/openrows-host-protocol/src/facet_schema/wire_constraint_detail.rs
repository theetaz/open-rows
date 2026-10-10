// SPDX-License-Identifier: Apache-2.0
//! Dedicated bounded facet DTOs; native API layouts never cross the transport.
use crate::{WireConstraintKind, WireForeignTarget};
use serde::{Deserialize, Serialize};
/// Transport representation of `ConstraintDetail`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WireConstraintDetail {
    /// Constraint name.
    pub name: String,
    /// Constraint kind.
    pub kind: WireConstraintKind,
    /// Server-formatted definition, e.g. `CHECK ((price > 0))`.
    pub definition: String,
    /// Constrained columns, in key order (empty for expression-only checks).
    #[serde(deserialize_with = "crate::bounded::list")]
    pub columns: Vec<String>,
    /// Referenced table of a foreign key.
    pub references: Option<WireForeignTarget>,
}
impl From<openrows_driver_api::schema::ConstraintDetail> for WireConstraintDetail {
    fn from(value: openrows_driver_api::schema::ConstraintDetail) -> Self {
        Self {
            name: value.name,
            kind: value.kind.into(),
            definition: value.definition,
            columns: value.columns,
            references: value.references.map(Into::into),
        }
    }
}
impl From<WireConstraintDetail> for openrows_driver_api::schema::ConstraintDetail {
    fn from(value: WireConstraintDetail) -> Self {
        Self {
            name: value.name,
            kind: value.kind.into(),
            definition: value.definition,
            columns: value.columns,
            references: value.references.map(Into::into),
        }
    }
}

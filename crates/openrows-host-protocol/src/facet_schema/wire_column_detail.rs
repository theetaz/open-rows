// SPDX-License-Identifier: Apache-2.0
//! Dedicated bounded facet DTOs; native API layouts never cross the transport.
use crate::WireGenerated;
use serde::{Deserialize, Serialize};
/// Transport representation of `ColumnDetail`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WireColumnDetail {
    /// Column name.
    pub name: String,
    /// Formatted type, e.g. `character varying(40)`.
    pub data_type: String,
    /// `false` for `NOT NULL` columns.
    pub nullable: bool,
    /// Default expression, or the generation expression of a stored column.
    pub default: Option<String>,
    /// Identity or generated-column kind.
    pub generated: WireGenerated,
    /// Comment, when set.
    pub comment: Option<String>,
}
impl From<openrows_driver_api::schema::ColumnDetail> for WireColumnDetail {
    fn from(value: openrows_driver_api::schema::ColumnDetail) -> Self {
        Self {
            name: value.name,
            data_type: value.data_type,
            nullable: value.nullable,
            default: value.default,
            generated: value.generated.into(),
            comment: value.comment,
        }
    }
}
impl From<WireColumnDetail> for openrows_driver_api::schema::ColumnDetail {
    fn from(value: WireColumnDetail) -> Self {
        Self {
            name: value.name,
            data_type: value.data_type,
            nullable: value.nullable,
            default: value.default,
            generated: value.generated.into(),
            comment: value.comment,
        }
    }
}

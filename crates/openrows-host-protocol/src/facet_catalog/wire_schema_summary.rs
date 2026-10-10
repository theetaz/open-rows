// SPDX-License-Identifier: Apache-2.0
//! Dedicated bounded facet DTOs; native API layouts never cross the transport.
use serde::{Deserialize, Serialize};
/// Transport representation of `SchemaSummary`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WireSchemaSummary {
    /// Schema name.
    pub name: String,
    /// Catalog schema (`pg_catalog`, `information_schema`), hidden by default.
    pub system: bool,
    /// Comment, when set.
    pub comment: Option<String>,
}
impl From<openrows_driver_api::catalog::SchemaSummary> for WireSchemaSummary {
    fn from(value: openrows_driver_api::catalog::SchemaSummary) -> Self {
        Self {
            name: value.name,
            system: value.system,
            comment: value.comment,
        }
    }
}
impl From<WireSchemaSummary> for openrows_driver_api::catalog::SchemaSummary {
    fn from(value: WireSchemaSummary) -> Self {
        Self {
            name: value.name,
            system: value.system,
            comment: value.comment,
        }
    }
}

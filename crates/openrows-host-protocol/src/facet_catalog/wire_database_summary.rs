// SPDX-License-Identifier: Apache-2.0
//! Dedicated bounded facet DTOs; native API layouts never cross the transport.
use serde::{Deserialize, Serialize};
/// Transport representation of `DatabaseSummary`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WireDatabaseSummary {
    /// Database name.
    pub name: String,
    /// Maintenance or provider database (e.g. `postgres`, `rdsadmin`), hidden by default.
    pub system: bool,
    /// Comment, when set.
    pub comment: Option<String>,
}
impl From<openrows_driver_api::catalog::DatabaseSummary> for WireDatabaseSummary {
    fn from(value: openrows_driver_api::catalog::DatabaseSummary) -> Self {
        Self {
            name: value.name,
            system: value.system,
            comment: value.comment,
        }
    }
}
impl From<WireDatabaseSummary> for openrows_driver_api::catalog::DatabaseSummary {
    fn from(value: WireDatabaseSummary) -> Self {
        Self {
            name: value.name,
            system: value.system,
            comment: value.comment,
        }
    }
}

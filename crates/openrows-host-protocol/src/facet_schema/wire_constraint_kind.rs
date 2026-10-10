// SPDX-License-Identifier: Apache-2.0
//! Dedicated bounded facet DTOs; native API layouts never cross the transport.
use serde::{Deserialize, Serialize};
/// Transport representation of `ConstraintKind`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum WireConstraintKind {
    /// `PRIMARY KEY`.
    Primary,
    /// `UNIQUE`.
    Unique,
    /// `CHECK`.
    Check,
    /// `FOREIGN KEY`.
    Foreign,
    /// `EXCLUDE`.
    Exclusion,
}
impl From<openrows_driver_api::schema::ConstraintKind> for WireConstraintKind {
    fn from(value: openrows_driver_api::schema::ConstraintKind) -> Self {
        match value {
            openrows_driver_api::schema::ConstraintKind::Primary => Self::Primary,
            openrows_driver_api::schema::ConstraintKind::Unique => Self::Unique,
            openrows_driver_api::schema::ConstraintKind::Check => Self::Check,
            openrows_driver_api::schema::ConstraintKind::Foreign => Self::Foreign,
            openrows_driver_api::schema::ConstraintKind::Exclusion => Self::Exclusion,
        }
    }
}
impl From<WireConstraintKind> for openrows_driver_api::schema::ConstraintKind {
    fn from(value: WireConstraintKind) -> Self {
        match value {
            WireConstraintKind::Primary => Self::Primary,
            WireConstraintKind::Unique => Self::Unique,
            WireConstraintKind::Check => Self::Check,
            WireConstraintKind::Foreign => Self::Foreign,
            WireConstraintKind::Exclusion => Self::Exclusion,
        }
    }
}

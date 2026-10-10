// SPDX-License-Identifier: Apache-2.0
//! Dedicated bounded facet DTOs; native API layouts never cross the transport.
use serde::{Deserialize, Serialize};
/// Transport representation of `ObjectKind`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum WireObjectKind {
    /// Ordinary or partitioned table.
    Table,
    /// View.
    View,
    /// Materialized view.
    MaterializedView,
    /// Foreign table.
    ForeignTable,
    /// Function (including window functions).
    Function,
    /// Stored procedure.
    Procedure,
    /// Sequence.
    Sequence,
    /// User-defined enum, composite, domain, range, alias or table type.
    Type,
}
impl From<openrows_driver_api::catalog::ObjectKind> for WireObjectKind {
    fn from(value: openrows_driver_api::catalog::ObjectKind) -> Self {
        match value {
            openrows_driver_api::catalog::ObjectKind::Table => Self::Table,
            openrows_driver_api::catalog::ObjectKind::View => Self::View,
            openrows_driver_api::catalog::ObjectKind::MaterializedView => Self::MaterializedView,
            openrows_driver_api::catalog::ObjectKind::ForeignTable => Self::ForeignTable,
            openrows_driver_api::catalog::ObjectKind::Function => Self::Function,
            openrows_driver_api::catalog::ObjectKind::Procedure => Self::Procedure,
            openrows_driver_api::catalog::ObjectKind::Sequence => Self::Sequence,
            openrows_driver_api::catalog::ObjectKind::Type => Self::Type,
        }
    }
}
impl From<WireObjectKind> for openrows_driver_api::catalog::ObjectKind {
    fn from(value: WireObjectKind) -> Self {
        match value {
            WireObjectKind::Table => Self::Table,
            WireObjectKind::View => Self::View,
            WireObjectKind::MaterializedView => Self::MaterializedView,
            WireObjectKind::ForeignTable => Self::ForeignTable,
            WireObjectKind::Function => Self::Function,
            WireObjectKind::Procedure => Self::Procedure,
            WireObjectKind::Sequence => Self::Sequence,
            WireObjectKind::Type => Self::Type,
        }
    }
}

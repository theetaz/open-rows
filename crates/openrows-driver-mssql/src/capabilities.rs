// SPDX-License-Identifier: Apache-2.0
//! What the SQL Server driver supports (docs/05 §2.1).
use openrows_driver_api::catalog::ObjectKind;
use openrows_driver_api::{Capabilities, Feature, Location};

/// A server holds databases, each with schemas; DDL runs inside
/// transactions. Cancel is the TDS attention signal; the process list can
/// only terminate other sessions (`KILL`), SQL Server has no way to cancel
/// another session's statement. Not claimed: bulk load (imports use
/// batched `INSERT`s), materialized views, database renames (they need
/// exclusive access), schema lifecycle (schemas cannot be renamed), column
/// character sets, and adding or removing `IDENTITY` on existing columns.
pub(crate) const CAPABILITIES: Capabilities = Capabilities {
    location: Location::Server,
    features: &[
        Feature::Cancel,
        Feature::BindParameters,
        Feature::MultipleDatabases,
        Feature::Schemas,
        Feature::Catalog,
        Feature::Types,
        Feature::Roles,
        Feature::RowEdits,
        Feature::StructureChanges,
        Feature::TransactionalDdl,
        Feature::Tls,
        Feature::Routes,
        Feature::StartupCommands,
        Feature::SchemaFingerprint,
        Feature::ProcessList,
        Feature::TableLifecycle,
        Feature::ViewEditing,
        Feature::TriggerEditing,
        Feature::RoutineEditing,
        Feature::DatabaseLifecycle,
        Feature::ForeignKeyEditing,
        Feature::CheckEditing,
        Feature::Comments,
        Feature::ColumnCollation,
        Feature::PartialIndexes,
    ],
    object_kinds: &[
        ObjectKind::Table,
        ObjectKind::View,
        ObjectKind::Function,
        ObjectKind::Procedure,
        ObjectKind::Sequence,
        ObjectKind::Type,
    ],
};

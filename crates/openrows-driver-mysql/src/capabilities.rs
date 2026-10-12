// SPDX-License-Identifier: Apache-2.0
//! What the `MySQL`/`MariaDB` driver supports (docs/05 §2.1).
use openrows_driver_api::catalog::ObjectKind;
use openrows_driver_api::{Capabilities, Feature, Location};

/// A database is its own only schema and DDL commits implicitly per
/// statement; structure changes are one `ALTER TABLE` (docs/05 §13).
/// Tables, views, triggers, routines and databases have editors; databases
/// cannot be renamed and there are no materialized views or partial indexes.
pub(crate) const CAPABILITIES: Capabilities = Capabilities {
    location: Location::Server,
    features: &[
        Feature::Cancel,
        Feature::BindParameters,
        Feature::MultipleDatabases,
        Feature::Catalog,
        Feature::Roles,
        Feature::RowEdits,
        Feature::StructureChanges,
        Feature::Tls,
        Feature::Routes,
        Feature::StartupCommands,
        Feature::SchemaFingerprint,
        Feature::ProcessList,
        Feature::TableLifecycle,
        Feature::KeyEditing,
        Feature::ColumnReordering,
        Feature::ViewEditing,
        Feature::TriggerEditing,
        Feature::RoutineEditing,
        Feature::DatabaseLifecycle,
        Feature::ForeignKeyEditing,
        Feature::CheckEditing,
        Feature::Comments,
        Feature::ColumnCollation,
        Feature::ColumnCharset,
        Feature::IdentityColumns,
    ],
    // `SEQUENCE` objects exist in `MariaDB` only.
    object_kinds: &[
        ObjectKind::Table,
        ObjectKind::View,
        ObjectKind::Function,
        ObjectKind::Procedure,
        ObjectKind::Sequence,
    ],
};

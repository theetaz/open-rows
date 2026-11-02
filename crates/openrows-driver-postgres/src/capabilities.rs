// SPDX-License-Identifier: Apache-2.0
//! What the `PostgreSQL` driver supports (docs/05 §2.1).
use openrows_driver_api::catalog::ObjectKind;
use openrows_driver_api::{Capabilities, Feature, Location};

/// Cancel requests, bound parameters, the full catalog, guarded edits,
/// generated structure changes, transactional DDL, TLS, routes, startup
/// commands, a catalog fingerprint, the process list, `COPY` bulk loading
/// and every object editor: every feature but per-column character sets
/// (the database encoding applies).
pub(crate) const CAPABILITIES: Capabilities = Capabilities {
    location: Location::Server,
    features: &[
        Feature::Cancel,
        Feature::BindParameters,
        Feature::MultipleDatabases,
        Feature::Schemas,
        Feature::Catalog,
        Feature::Types,
        Feature::Extensions,
        Feature::Roles,
        Feature::RowEdits,
        Feature::StructureChanges,
        Feature::TransactionalDdl,
        Feature::Tls,
        Feature::Routes,
        Feature::StartupCommands,
        Feature::SchemaFingerprint,
        Feature::ProcessList,
        Feature::BulkLoad,
        Feature::TableLifecycle,
        Feature::KeyEditing,
        Feature::TypeUsing,
        Feature::ConcurrentIndexes,
        Feature::ViewEditing,
        Feature::MaterializedViews,
        Feature::TriggerEditing,
        Feature::RoutineEditing,
        Feature::DatabaseLifecycle,
        Feature::DatabaseRename,
        Feature::SchemaLifecycle,
        Feature::ForeignKeyEditing,
        Feature::CheckEditing,
        Feature::Comments,
        Feature::ColumnCollation,
        Feature::IdentityColumns,
        Feature::PartialIndexes,
        Feature::Notices,
        Feature::Explain,
    ],
    object_kinds: &ObjectKind::ALL,
};

/// What a `CockroachDB` server offers through this driver, as measured by the
/// conformance suite (`just test-pg cockroach`, docs/05 §12.1).
pub(crate) const COCKROACH_CAPABILITIES: Capabilities = Capabilities {
    location: Location::Server,
    features: &[
        Feature::Cancel,
        Feature::BindParameters,
        Feature::MultipleDatabases,
        Feature::Schemas,
        Feature::Catalog,
        Feature::Types,
        Feature::Extensions,
        Feature::Roles,
        Feature::RowEdits,
        Feature::StructureChanges,
        Feature::Tls,
        Feature::Routes,
        Feature::StartupCommands,
        Feature::TableLifecycle,
    ],
    object_kinds: &[
        ObjectKind::Table,
        ObjectKind::View,
        ObjectKind::Sequence,
        ObjectKind::Type,
    ],
};
